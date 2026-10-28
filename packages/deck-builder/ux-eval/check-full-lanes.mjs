#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {UX_EVAL_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot} from './lib/browser.mjs';
import {ROW_LABELS} from '../web/src/domain/table.ts';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8853',baseline=process.argv.includes('--baseline'),out=opt('out')??join(UX_EVAL_ROOT,'manual',baseline?'full-lane-baseline':'full-lane');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'uxeval_user=1'}})).json());
const deckBefore=(await api('/api/decks/2')).state,browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{for(const width of opt('width')?[Number(opt('width'))]:baseline?[1440]:[1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page),moves=[];
 page.on('request',request=>{if(request.method()==='POST'&&request.url().endsWith('/playtest/actions')){const action=request.postDataJSON()?.action;if(action?.type==='move')moves.push(action);}});
 await page.goto(origin+'/deck/2/playtest');await settle(page,watch);await page.keyboard.press('Escape');await settle(page,watch);
 const read=async()=>(await api('/api/decks/2/playtest')).playtest;
 const initial=await read();assert(initial&&initial.session.frame.opening.stage==='playing');assert.equal(initial.session.frame.zones.battlefield.length,0,'use a fresh fixture copy with an empty saved board');
 const hand=page.getByRole('region',{name:'Hand',exact:true}),field=page.locator('[aria-label="Battlefield"]'),lanes=field.locator(':scope > section'),rowKeys=Object.keys(ROW_LABELS);assert.equal(await lanes.count(),3);
 const rect=async index=>{
  const bounds=await field.boundingBox();
  if(index===0||index===2){await page.mouse.move(bounds.x+bounds.width/2,bounds.y+bounds.height/2);await page.mouse.wheel(0,index===0?-5000:5000);await settle(page,watch);}
  const box=await lanes.nth(index).boundingBox(),battlefield=await field.evaluate(el=>{const b=el.getBoundingClientRect();return {x:b.left,y:b.top,width:el.clientWidth,height:el.clientHeight};});
  const y=Math.max(battlefield.y,index===0?battlefield.y:box.y),bottom=Math.min(box.y+box.height,battlefield.y+battlefield.height);
  assert(bottom-y>10,'the requested lane has a visible bounded area');return {x:battlefield.x,y,width:battlefield.width,height:bottom-y};
 };
 const point=(r,kind)=>({x:kind==='left'?r.x+3:kind==='right'?r.x+r.width-4:r.x+r.width*.65,y:kind==='top'?r.y+3:kind==='bottom'?r.y+r.height-4:r.y+r.height/2});
 const start=async p=>{
  const source=hand.locator('[data-object-id]').first(),id=await source.getAttribute('data-object-id'),b=await source.locator('header').boundingBox();assert(b&&b.y>=0&&b.y<VIEWPORTS[width].height);
  await page.mouse.move(b.x+Math.min(40,b.width/2),b.y+12);await page.mouse.down();await page.mouse.move(b.x+Math.min(40,b.width/2)+8,b.y+18,{steps:3});await page.mouse.move(p.x,p.y,{steps:12});await page.mouse.move(p.x+1,p.y,{steps:2});return id;
 };
 const finish=async()=>{await page.mouse.up();await settle(page,watch);};
 const undo=async frame=>{await page.getByRole('button',{name:'Undo (Z)',exact:true}).click();await settle(page,watch);assert.deepEqual((await read()).session.frame,frame,'Undo restores exact saved order and object state');};
 const drop=async(index,kind,highlight=true)=>{
  const before=(await read()).session.frame,r=await rect(index),p=point(r,kind),count=moves.length,id=await start(p);
  if(highlight)assert.equal(await lanes.nth(index).getAttribute('data-drop-active'),'true','the actual full lane highlights during a native drag');
  if(index===0&&kind==='top')await shoot(page,join(out,`active-top-${width}-${count}.png`),{full:false});
  await finish();assert.equal(moves.length-count,1,'one native drop makes exactly one move request');const after=await read();assert(after.session.frame.zones.battlefield.includes(id));assert.equal(after.session.frame.objects[id].row,rowKeys[index]);assert(!after.session.frame.zones.hand.includes(id));
  assert.equal(await page.locator('[data-drop-active]').count(),0,'drop highlight resets after release');return {before,id,r};
 };
 if(baseline){
  const own=await lanes.first().boundingBox();await start({x:own.x+own.width*.65,y:own.y+own.height-10});await finish();assert.equal(moves.length,1,'the old short strip accepts a native drop');
  const bounded=await rect(0),actual=await lanes.first().boundingBox();assert(actual.y>bounded.y+5,'the visible top area lies outside the target element');const before=(await read()).session.frame,count=moves.length;
  await start(point(bounded,'top'));await finish();assert.equal(moves.length,count,'the old blank lane top rejects the same native drag');assert.deepEqual((await read()).session.frame,before);await shoot(page,join(out,`reproduced-${width}.png`),{full:false});results.push({width,reproduced:true,visible_lane:bounded,actual_target:actual,blank_top_ignored:true});
 }else{
  const cases=[];
  for(const scene of ['empty','populated']){
   let setup=0;
   if(scene==='populated')for(let i=0;i<3;i++){await drop(i,'right');setup++;}
   for(let i=0;i<3;i++)for(const kind of ['top','center','left','right','bottom']){
    const trial=await drop(i,kind);cases.push({scene,row:rowKeys[i],point:kind});await undo(trial.before);
   }
   if(scene==='populated'){
    const pinned=lanes.first().locator('[data-object-id]').first();await pinned.click({position:{x:20,y:20}});const draft=page.getByRole('textbox',{name:'Playtest observation',exact:true});await draft.fill('Keep this observation while dragging.');await settle(page,watch);const pinnedId=await pinned.getAttribute('data-object-id');const before=(await read()).session.frame,count=moves.length,r=await rect(0),id=await start(point(r,'right')),card=lanes.first().locator('[data-object-id]').first(),b=await card.boundingBox();
    for(const p of [{x:b.x+b.width/2,y:b.y+15},{x:b.x+8,y:b.y+b.height/2},point(r,'center')]){await page.mouse.move(p.x,p.y,{steps:4});assert.equal(await lanes.first().getAttribute('data-drop-active'),'true','highlight does not flicker across child cards');}
    await page.mouse.move(b.x+b.width/2,b.y+15,{steps:3});await finish();assert.equal(moves.length-count,1,'child reorder does not also trigger the lane move');assert((await read()).session.frame.zones.battlefield.includes(id));await undo(before);assert.equal(await draft.inputValue(),'Keep this observation while dragging.');assert.equal((await read()).session.ui.pinned,pinnedId);
   }
   const unchanged=(await read()).session.frame,count=moves.length;
   await start(point(await rect(0),'center'));await page.keyboard.press('Escape');await page.mouse.up();await settle(page,watch);assert.equal(moves.length,count);assert.deepEqual((await read()).session.frame,unchanged);assert.equal(await page.locator('[data-drop-active]').count(),0);
   await start({x:VIEWPORTS[width].width/2,y:12});await finish();assert.equal(moves.length,count);assert.deepEqual((await read()).session.frame,unchanged,'outside the table is not a catch-all drop target');
   await shoot(page,join(out,`${scene}-${width}.png`),{full:false});await page.reload();await settle(page,watch);assert.deepEqual((await read()).session.frame,unchanged,'the table resumes exact lane membership and order');
   for(let i=0;i<setup;i++)await page.getByRole('button',{name:'Undo (Z)',exact:true}).click().then(()=>settle(page,watch));assert.deepEqual((await read()).session.frame,initial.session.frame);
  }
  const card=hand.locator('[data-object-id]').first();await card.focus();await card.press('Enter');await settle(page,watch);assert.equal(await card.getAttribute('data-selected'),'true');await card.press('Shift+F10');await page.getByRole('menu').waitFor();await page.keyboard.press('Escape');assert.equal((await axe(page)).length,0);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);results.push({width,cases,native_drag_input:true,all_three_full_lanes:true,child_highlight_stable:true,exactly_once:true,cancel_outside_undo_resume:true,keyboard_selection_menu:true,axe:0});
 }
 assert.deepEqual((await api('/api/decks/2')).state,deckBefore,'manual table drag actions never edit the saved deck');await context.close();console.log(`Full lanes ${width} ${baseline?'reproduced':'passed'}`);
}writeFileSync(join(out,'qa.json'),JSON.stringify({results,deck_unchanged:true},null,2)+'\n');}finally{await browser.close();}
