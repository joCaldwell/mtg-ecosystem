#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {CHECKS_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,scrollUntilVisible,axe,shoot} from './lib/browser.mjs';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8855',baseline=process.argv.includes('--baseline'),out=opt('out')??join(CHECKS_ROOT,'manual',baseline?'long-maybe-baseline':'long-maybe');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const fixture=JSON.parse(readFileSync(join(CHECKS_ROOT,'long-note-fixture-v1/coverage.json'),'utf8')),deckId=fixture.deck;
const api=async(path,method='GET',body,url=origin)=>{const response=await fetch(url+path,{method,headers:{Cookie:'checks_user=1',Origin:url,'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});assert.equal(response.status,200);return response.json();};
const before=(await api(`/api/decks/${deckId}`)).state,notes=before.cards.filter(c=>c.maybe),long=notes.find(c=>c.mana_cost&&c.maybe_note.includes('\n\n')),costless=notes.find(c=>!c.mana_cost&&c.maybe_note.includes('\n\n')),reference=before.cards.find(c=>!c.maybe&&c.role==='card');assert.equal(notes.length,20);assert(long&&costless&&reference);
const browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{for(const width of process.argv.includes('--promotion-only')?[]:[1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+`/deck/${deckId}`);await settle(page,watch);const panel=page.getByRole('complementary',{name:'Deck panel',exact:true}),resize=page.getByRole('separator',{name:'Resize the deck panel',exact:true}),maybe=page.getByRole('region',{name:'Maybe list',exact:true}),rows=maybe.locator('[data-oracle-id]');
 const originalOrder=await rows.evaluateAll(nodes=>nodes.map(n=>n.dataset.oracleId));
 const defaultWidth=Math.round((await panel.boundingBox()).width);
 if(!baseline){await panel.getByRole('button',{name:'Edit the thesis',exact:true}).click();await panel.getByRole('textbox',{name:'Thesis',exact:true}).fill('Keep this unsaved brief while browsing long shortlist notes.');}
 const sizes=[];
 for(const size of [340,392,640]){
  if(size===392){
   // The default grows on wide windows (ARCHITECTURE.md). Verify reset,
   // then drag to the actual 392px case rather than assuming they coincide.
   await resize.dblclick();await settle(page,watch);assert.equal(Math.round((await panel.boundingBox()).width),defaultWidth,'reset restores the fluid default');
   const handle=await resize.boundingBox(),x=handle.x+handle.width/2,y=handle.y+handle.height/2;
   await page.mouse.move(x,y);await page.mouse.down();await page.mouse.move(x+defaultWidth-size,y);await page.mouse.up();
  }else {await resize.focus();await resize.press(size===340?'Home':'End');}await settle(page,watch);assert.equal(Math.round((await panel.boundingBox()).width),size);
  await scrollUntilVisible(page,maybe,VIEWPORTS[width],{top:true});assert.equal(await rows.count(),20);assert.deepEqual(await rows.evaluateAll(nodes=>nodes.map(n=>n.dataset.oracleId)),originalOrder);
  const boxes=await rows.evaluateAll(nodes=>nodes.map(n=>{const b=n.getBoundingClientRect();return {id:n.dataset.oracleId,x:b.x,y:b.y,width:b.width,height:b.height,text:n.innerText};})),span=Math.max(...boxes.map(b=>b.y+b.height))-Math.min(...boxes.map(b=>b.y));
  if(!baseline){assert(boxes.every(b=>b.height<=64),'note prose cannot make a shortlist row tall');assert(boxes.every(b=>!b.text.includes('Option ')&&!b.text.includes('After the next playtest')));assert.equal(await maybe.locator('[class*="_purpose_"], [class*="_note_"]').count(),0);}
  await shoot(page,join(out,`list-${width}-${size}.png`),{full:false});
  if(!baseline){
   const row=maybe.locator(`[data-oracle-id="${long.oracle_id}"]`),hit=row.getByRole('button',{name:long.name,exact:true});await scrollUntilVisible(page,row,VIEWPORTS[width]);await hit.focus();await hit.press('Space');await settle(page,watch);
   assert.equal(await hit.evaluate(el=>document.activeElement===el),true,'opening from list keeps keyboard browsing focus');const note=panel.getByRole('region',{name:'Maybe note',exact:true});await note.waitFor();
   assert.deepEqual(await note.locator('p').allTextContents(),long.maybe_note.split(/\n\s*\n/).map(p=>p.replace(/\[\[|\]\]/g,'')),'all stored paragraphs and card names are readable without edit mode');
   assert.equal(await note.getByRole('textbox').count(),0);assert.equal(await note.getByRole('button',{name:reference.name,exact:true}).count(),1);
   const viewport=await panel.locator('[data-panel-body]').boundingBox(),noteBox=await note.boundingBox();assert(noteBox.y>=viewport.y-1&&noteBox.y+noteBox.height<=viewport.y+viewport.height+1,'full note is visible in the card-details reading area');
   await shoot(page,join(out,`note-${width}-${size}.png`),{full:false});
   await note.getByRole('button',{name:reference.name,exact:true}).click();await settle(page,watch);await panel.getByRole('heading',{name:reference.name,exact:true}).waitFor();await panel.getByRole('button',{name:'Back to Brief',exact:true}).click();await settle(page,watch);
   assert.equal(await hit.evaluate(el=>document.activeElement===el),true,'Back returns to the first Maybe trigger across linked-card inspection');assert.equal(await panel.getByRole('textbox',{name:'Thesis',exact:true}).inputValue(),'Keep this unsaved brief while browsing long shortlist notes.');
   const price=row.locator('[class*="_price_"]').last(),priceBox=await price.boundingBox();await page.mouse.click(priceBox.x+priceBox.width/2,priceBox.y+priceBox.height/2);await settle(page,watch);await note.waitFor();await page.keyboard.press('Escape');await settle(page,watch);
   const emptyRow=maybe.locator(`[data-oracle-id="${costless.oracle_id}"]`);await scrollUntilVisible(page,emptyRow,VIEWPORTS[width]);await emptyRow.getByRole('button',{name:costless.name,exact:true}).focus();await page.keyboard.press('Enter');await settle(page,watch);await panel.getByRole('region',{name:'Maybe note',exact:true}).waitFor();await panel.getByRole('button',{name:'Back to Brief',exact:true}).click();await settle(page,watch);
  }
  sizes.push({panel:size,max_row_height:Math.max(...boxes.map(b=>b.height)),list_height:span,columns:new Set(boxes.map(b=>Math.round(b.x))).size});
 }
 if(!baseline){
  const row=maybe.locator(`[data-oracle-id="${long.oracle_id}"]`);await scrollUntilVisible(page,row,VIEWPORTS[width]);await row.getByRole('button',{name:long.name,exact:true}).click({modifiers:['Control']});await settle(page,watch);assert.equal(await page.getByRole('toolbar',{name:'Selected cards',exact:true}).count(),0);await panel.getByRole('button',{name:'Back to Brief',exact:true}).click();
  const main=page.locator('[data-deck-list="true"] [class*="_cols_"] [data-oracle-id]').first();await scrollUntilVisible(page,main,VIEWPORTS[width]);await main.click({modifiers:['Control']});await page.getByRole('toolbar',{name:'Selected cards',exact:true}).waitFor();await page.getByRole('toolbar',{name:'Selected cards',exact:true}).getByRole('button',{name:/^Clear/}).click();assert.deepEqual(await axe(page),[]);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 }
 assert.deepEqual((await api(`/api/decks/${deckId}`)).state,before,'list and detail presentation preserve exact notes, links, assignments, revision and membership');results.push({width,sizes,notes_exact:true,order_exact:true,full_note_read_mode:!baseline,linked_card_back_draft_focus:!baseline});await context.close();console.log(`Long Maybe ${width} ${baseline?'measured':'passed'}`);
}
let promotionUndo=null;
if(opt('baseline-origin')){
 const other=opt('baseline-origin');assert.equal(new URL(other).hostname,'127.0.0.1');const baselineBefore=(await api(`/api/decks/${deckId}`,'GET',undefined,other)).state;
 assert.deepEqual(baselineBefore.cards,before.cards);
 const context=await newContext(browser,{origin,vp:VIEWPORTS[1440],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+`/deck/${deckId}`);await settle(page,watch);const hit=page.getByRole('region',{name:'Maybe list',exact:true}).locator(`[data-oracle-id="${long.oracle_id}"]`).getByRole('button',{name:long.name,exact:true});await scrollUntilVisible(page,hit,VIEWPORTS[1440]);await hit.click();await settle(page,watch);const add=page.getByRole('button',{name:'Add to the deck',exact:true});await scrollUntilVisible(page,add,VIEWPORTS[1440]);await add.click();await settle(page,watch);
 const promoted=(await api(`/api/decks/${deckId}`)).state,entry=promoted.log.find(log=>!before.log.some(old=>old.id===log.id));assert(entry);const promotedCard=promoted.cards.find(c=>c.oracle_id===long.oracle_id);assert.equal(promotedCard.maybe,0);assert.equal(promotedCard.slot_id,long.slot_id);assert.deepEqual(promotedCard.secondary_slot_ids,long.secondary_slot_ids);
 const undone=await api(`/api/decks/${deckId}/log/${entry.id}/undo`,'POST');assert.deepEqual(undone.skipped,[]);const restored=undone.state.cards.find(c=>c.oracle_id===long.oracle_id);assert.equal(restored.maybe,1);assert.equal(restored.slot_id,long.slot_id);assert.deepEqual(restored.secondary_slot_ids,long.secondary_slot_ids);
 const basePromoted=(await api(`/api/decks/${deckId}/cards/${long.oracle_id}`,'PATCH',{maybe:false},other)).state,baseEntry=basePromoted.log.find(log=>!baselineBefore.log.some(old=>old.id===log.id));const baseUndone=await api(`/api/decks/${deckId}/log/${baseEntry.id}/undo`,'POST',undefined,other);assert.deepEqual(undone.state.cards,baseUndone.state.cards,'presentation retains the exact existing promotion/undo behavior');
 // Restore fixture prose after intentionally exercising the pre-existing
 // promotion behavior (which clears its note on promotion and undo).
 for(const url of [origin,other])await api(`/api/decks/${deckId}/cards/${long.oracle_id}`,'PATCH',{maybe_note:long.maybe_note},url);
 assert.deepEqual((await api(`/api/decks/${deckId}`)).state.cards,before.cards);promotionUndo={native_promotion:true,primary_secondary_membership_retained:true,exact_baseline_behavior:true,existing_note_clear_behavior_unchanged:true,fixture_note_restored:true};await context.close();
}
writeFileSync(join(out,'qa.json'),JSON.stringify({baseline,promotionUndo,fixture:join(CHECKS_ROOT,'long-note-fixture-v1/seed.db'),results,stored_data_unchanged:true},null,2)+'\n');}finally{await browser.close();}
