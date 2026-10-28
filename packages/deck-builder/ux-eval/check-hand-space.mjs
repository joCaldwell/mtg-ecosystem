#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {UX_EVAL_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot} from './lib/browser.mjs';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8853',variant=opt('variant')??'candidate',out=opt('out')??join(UX_EVAL_ROOT,'manual',`hand-space-${variant}`);
assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'uxeval_user=1'}})).json());
const before=(await api('/api/decks/2')).state,browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{for(const width of [1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+'/deck/2/playtest');await settle(page,watch);
 if(!process.argv.includes('--resume')){
  if(!await page.getByRole('button',{name:'Shuffle and draw seven',exact:true}).count()){await page.getByRole('button',{name:'More',exact:true}).click();await page.getByRole('menuitem',{name:/^New game/}).click();}
  await page.getByRole('button',{name:'Shuffle and draw seven',exact:true}).click();await settle(page,watch);
  await page.getByRole('button',{name:'Keep hand',exact:true}).click();await settle(page,watch);
 }
 const hand=page.getByRole('region',{name:'Hand',exact:true}),cards=hand.locator('[data-object-id]'),scroll=hand.locator('[data-table-cards]'),table=hand.locator('..');
 const read=async()=>(await api('/api/decks/2/playtest')).playtest;
 const kept=await read();assert.equal(await cards.count(),7);assert.equal(kept.session.frame.zones.battlefield.length,0);
 const geometry=async()=>({hand:await hand.boundingBox(),panel:await page.getByRole('complementary',{name:'Inspection panel'}).boundingBox(),scroll:await scroll.evaluate(el=>({height:el.clientHeight,content:el.scrollHeight,width:el.clientWidth,contentWidth:el.scrollWidth,y:el.scrollTop,overflowY:getComputedStyle(el).overflowY})),empty:await table.evaluate(el=>el.className.includes('emptyBattlefield'))});
 const empty=await geometry();if(variant==='candidate')assert(empty.empty);
 const battlefield=table.locator('[aria-label="Battlefield"]');assert.equal(await battlefield.locator(':scope > section').count(),3);assert.equal(await battlefield.getByRole('heading').count(),0,'three simple rows remain without visible headings');
 const rules=await cards.locator('[class*="_rules_"]').evaluateAll(nodes=>nodes.map(n=>({text:n.textContent,font:parseFloat(getComputedStyle(n).fontSize)})));assert(rules.every(r=>r.font===15));
 await shoot(page,join(out,`empty-board-${width}.png`),{full:false});
 if(empty.scroll.content>empty.scroll.height+1){const b=await scroll.boundingBox();await page.mouse.move(b.x+b.width/2,b.y+b.height/2);await page.mouse.wheel(0,350);await settle(page,watch);const y=await scroll.evaluate(el=>el.scrollTop);if(variant==='candidate')assert(y>0,'natural vertical scrolling reaches full rules');}
 await cards.first().focus();await cards.first().press('Enter');await settle(page,watch);
 const first=cards.first(),id=await first.getAttribute('data-object-id'),draft=page.getByRole('textbox',{name:'Playtest observation',exact:true});await draft.fill('Keep this local observation while rearranging the table.');
 const afterDraft=await read();assert.deepEqual(afterDraft.session.frame,kept.session.frame);
 const panelWidths=[];
 if(process.argv.includes('--resize')){
  const resize=page.getByRole('separator',{name:'Resize inspection panel',exact:true});
  for(const key of ['Home','End']){
   await resize.focus();await resize.press(key);await settle(page,watch);const g=await geometry();panelWidths.push(g.panel.width);assert(Math.abs(g.panel.width-(key==='Home'?340:Math.min(640,width*.34)))<1);
   assert.equal(await draft.inputValue(),'Keep this local observation while rearranging the table.');assert.equal(await first.getAttribute('data-selected'),'true');assert.equal(await cards.count(),7);assert.deepEqual((await read()).session.frame,kept.session.frame);await shoot(page,join(out,`panel-${Math.round(g.panel.width)}-${width}.png`),{full:false});
  }
  await resize.dblclick();await settle(page,watch);assert.equal(Math.round((await geometry()).panel.width),400);
 }
 await page.getByRole('navigation',{name:'Table zones'}).getByRole('button',{name:/^Library/}).click();await settle(page,watch);if(variant==='candidate')assert.equal((await geometry()).empty,false);
 await page.getByRole('button',{name:'Close viewer',exact:true}).click();await settle(page,watch);if(variant==='candidate')assert.equal((await geometry()).empty,true);
 assert.equal(await draft.inputValue(),'Keep this local observation while rearranging the table.');assert.deepEqual((await read()).session.frame,kept.session.frame);
 await first.focus();await first.press('Shift+F10');await page.getByRole('menuitem',{name:'Move to battlefield',exact:true}).click();await settle(page,watch);
 const placed=await read();assert.deepEqual(placed.session.frame.zones.battlefield,[id]);const occupied=await geometry();assert.equal(occupied.empty,false);
 assert.equal(await draft.inputValue(),'Keep this local observation while rearranging the table.');assert.equal(await page.locator(`[data-object-id="${id}"]`).first().getAttribute('data-selected'),'true');
 await shoot(page,join(out,`occupied-board-${width}.png`),{full:false});
 await page.getByRole('button',{name:'Undo (Z)',exact:true}).click();await settle(page,watch);assert.deepEqual((await read()).session.frame,kept.session.frame);if(variant==='candidate')assert.equal((await geometry()).empty,true);
 assert.equal(await draft.inputValue(),'Keep this local observation while rearranging the table.');
 await draft.fill('');await page.reload();await settle(page,watch);assert.deepEqual((await read()).session.frame,kept.session.frame);
 assert.deepEqual((await api('/api/decks/2')).state,before,'table presentation and actions do not edit deck membership, assignments or revision');
 const violations=await axe(page);assert.deepEqual(violations,[]);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 results.push({width,empty,occupied,panelWidths,full_oracle_rules:rules,three_unlabelled_rows:true,selection_draft_viewer_undo_resume:true,deck_unchanged:true,axe:0});await context.close();console.log(`${variant} hand space ${width} passed`);
}writeFileSync(join(out,'qa.json'),JSON.stringify({variant,results},null,2)+'\n');}finally{await browser.close();}
