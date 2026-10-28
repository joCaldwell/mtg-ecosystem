#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {UX_EVAL_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot} from './lib/browser.mjs';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8853',out=opt('out')??join(UX_EVAL_ROOT,'manual','hand-empty');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'uxeval_user=1'}})).json());const before=await api('/api/decks/2');
const browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{
 for(const width of [1280,1440,1920,2560]){
  const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
  await page.goto(origin+'/deck/2/playtest');await settle(page,watch);
  if(!await page.getByRole('button',{name:'Shuffle and draw seven',exact:true}).count()){await page.getByRole('button',{name:'More',exact:true}).click();await page.getByRole('menuitem',{name:/^New game/}).click();}
  await page.getByRole('button',{name:'Shuffle and draw seven',exact:true}).click();await settle(page,watch);
  const hand=page.getByRole('region',{name:'Hand',exact:true}),cards=hand.locator('[data-object-id]'),scroller=hand.locator('[data-table-cards]');assert.equal(await cards.count(),7);
  const snapshot=await api('/api/decks/2/playtest'),seen=new Set();
  const observe=async()=>{for(const id of await scroller.evaluate(el=>{const r=el.getBoundingClientRect();return [...el.querySelectorAll('[data-object-id]')].filter(n=>{const b=n.getBoundingClientRect();return b.left>=r.left-1&&b.right<=r.right+1&&b.top<r.bottom&&b.bottom>r.top;}).map(n=>n.dataset.objectId);}))seen.add(id);};
  await observe();await shoot(page,join(out,`opening-${width}.png`),{full:false});
  const geometry=await scroller.evaluate(el=>({horizontal:el.scrollWidth>el.clientWidth+1,vertical:el.scrollHeight>el.clientHeight+1}));
  const next=hand.getByRole('button',{name:'Next hand cards',exact:true}),previous=hand.getByRole('button',{name:'Previous hand cards',exact:true});let forward=0;
  if(geometry.horizontal||geometry.vertical){assert.equal(await previous.isDisabled(),true,'Previous reaches the start');while(await next.isEnabled()&&forward<15){await next.focus();await next.press('Enter');await settle(page,watch);await observe();forward++;}assert(forward>0);assert.equal(await next.isDisabled(),true,'Next reaches the end');assert.equal(await previous.isEnabled(),true);await shoot(page,join(out,`last-cards-${width}.png`),{full:false});let reverse=0;while(await previous.isEnabled()&&reverse<15){await previous.focus();await previous.press('Space');await settle(page,watch);reverse++;}assert.equal(await previous.isDisabled(),true,'Previous reaches the start');}else assert.equal(await next.count(),0);
  assert.equal(seen.size,7,'all seven cards become visible without mutating or inspecting them');assert.deepEqual(await api('/api/decks/2/playtest'),snapshot,'scroll controls do not change frame, selection, observation or table history');
  await page.getByRole('button',{name:'Keep hand',exact:true}).click();await settle(page,watch);
  const kept=await api('/api/decks/2/playtest');let playingMoves=0;
  if(await next.count()){
   while(await next.isEnabled()&&playingMoves<15){await next.focus();await next.press('Enter');await settle(page,watch);playingMoves++;}
   assert.equal(await next.isDisabled(),true,'playing hand can reach its far edge, including vertical rules-text overflow');
   let backMoves=0;while(await previous.isEnabled()&&backMoves<15){await previous.focus();await previous.press('Space');await settle(page,watch);backMoves++;}assert.equal(await previous.isDisabled(),true);
  }
  assert.deepEqual(await api('/api/decks/2/playtest'),kept);await shoot(page,join(out,`playing-${width}.png`),{full:false});
  await cards.last().focus();await cards.last().press('Enter');await settle(page,watch);assert.equal(await cards.last().getAttribute('data-selected'),'true');
  const identity=await cards.last().locator('header').boundingBox(),viewport=await scroller.boundingBox();assert(identity&&viewport);
  assert(identity.y>=viewport.y-1&&identity.y+identity.height<=viewport.y+viewport.height+1,'selected card name, mana and type remain visible after native focus scroll');
  await shoot(page,join(out,`selected-identity-${width}.png`),{full:false});await cards.last().press('Shift+F10');await page.getByRole('menu').waitFor();await page.keyboard.press('Escape');await settle(page,watch);assert.equal((await axe(page)).length,0);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  results.push({width,geometry,forward_clicks:forward,playing_moves:playingMoves,all_seven_cards_visible:true,scroll_is_presentation_only:true,native_enter_space:true,selection_and_menu_preserved:true,axe:0,overflow:false});await context.close();
  const empty=await newContext(browser,{origin,vp:VIEWPORTS[width],user:2,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),p=await empty.newPage(),w=watchPage(p);
  await p.goto(origin+'/inbox');await settle(p,w);await p.getByRole('heading',{name:'Nothing is waiting on you',level:2}).waitFor();assert.equal(await p.locator('[class*="_split_"]').count(),0);
  const h1=await p.getByRole('heading',{name:'Inbox',level:1}).boundingBox(),emptyBox=await p.getByRole('heading',{name:'Nothing is waiting on you'}).locator('..').boundingBox();assert(Math.abs(h1.x-emptyBox.x)<1,'empty state aligns with the content frame instead of an unused list column');await shoot(p,join(out,`inbox-empty-${width}.png`),{full:false});
  await p.getByRole('button',{name:'Show proposal history',exact:true}).click();await p.getByText('No proposal history yet',{exact:true}).waitFor();await p.getByRole('button',{name:'Hide proposal history',exact:true}).click();
  await p.goto(origin+'/inbox?entry=proposal-999999');await settle(p,w);await p.getByRole('heading',{name:'This item is no longer waiting',level:2}).waitFor();await p.getByRole('link',{name:'Back to inbox',exact:true}).last().click();await settle(p,w);await p.getByRole('heading',{name:'Nothing is waiting on you',level:2}).waitFor();
  await p.goto(origin+'/activity');await settle(p,w);assert.equal(await p.getByRole('navigation',{name:'Activity pages',exact:true}).count(),0);await shoot(p,join(out,`activity-empty-${width}.png`),{full:false});assert.equal((await axe(p)).length,0);assert.equal(w.errors.length,0);await empty.close();console.log(`Hand and empty states ${width} passed`);
 }
 assert.deepEqual(await api('/api/decks/2'),before);writeFileSync(join(out,'qa.json'),JSON.stringify({results,deck_unchanged:true,empty_inbox_uses_content_frame:true,empty_history_and_stale_link_return:true,empty_activity_has_no_dead_pagination:true},null,2)+'\n');
}finally{await browser.close();}
