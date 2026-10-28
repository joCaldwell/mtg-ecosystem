#!/usr/bin/env node
// Real-input presentation/scope QA on serve.ts's disposable fixtures only.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { chromium } from 'playwright-core';
import { UX_EVAL_ROOT } from './lib/paths.mjs';
import { BROWSER_ARGS, VIEWPORTS, newContext, watchPage, settle, scrollUntilVisible, axe } from './lib/browser.mjs';
const option = name => { const i = process.argv.indexOf('--'+name); return i < 0 ? undefined : process.argv[i+1]; };
const origin = option('origin') ?? 'http://127.0.0.1:8854';
assert.equal(new URL(origin).hostname, '127.0.0.1', 'disposable local fixtures only');
const out = option('out') ?? join(UX_EVAL_ROOT, 'manual', 'review-clarity'); mkdirSync(out,{recursive:true});
const api = async path => { const r=await fetch(origin+path,{headers:{Cookie:'uxeval_user=1'}}); assert.equal(r.status,200); return r.json(); };
const before=(await api('/api/decks/2')).state, stats=await api('/api/stats');
const membership = state => state.cards.map(c=>({id:c.oracle_id,maybe:c.maybe,slot:c.slot_id,secondary:c.secondary_slot_ids,quantity:c.quantity,role:c.role,note:c.maybe_note}));
const selected=before.proposals.find(p=>p.items.length===1), originNote="From the pasted list's Sideboard";
assert(selected); const imported=before.cards.filter(c=>c.maybe&&c.maybe_note===originNote), custom=before.cards.filter(c=>c.maybe&&c.maybe_note&&c.maybe_note!==originNote);
assert(imported.length>1);
const browser=await chromium.launch({args:BROWSER_ARGS}), results=[];
try {
 for(const width of [1280,1440,1920,2560]){
  const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
  await page.goto(`${origin}/inbox?entry=proposal-${selected.id}`); await settle(page,watch);
  // Select through the actual inbox control if a deep link is not selected.
  const entry=page.locator(`a[href*="proposal-${selected.id}"]`).first();
  if(await entry.count()) {await entry.click();await settle(page,watch);}
  const changes=page.getByText('Changes in this proposal',{exact:true}); await changes.waitFor();
  for(const reason of new Set(selected.items.map(item=>item.reason).filter(Boolean))){
   const plain=reason.replace(/\[\[|\]\]/g,'');
   const actual=page.getByText(plain,{exact:true});await actual.waitFor();assert.equal(await actual.count(),1,'the selected proposal preserves actual agent reason context');
  }
  const summary=page.locator('summary').filter({hasText:`Deck impact if all ${before.proposals.length} pending proposals are accepted`});await summary.waitFor();
  const aggregate=summary.locator('..'); assert.equal(await aggregate.getAttribute('open'),null);
  assert(await changes.evaluate((el,text)=>!!(el.compareDocumentPosition([...document.querySelectorAll('summary')].find(s=>s.textContent===text))&Node.DOCUMENT_POSITION_FOLLOWING),await summary.textContent()));
  const reason=page.getByRole('textbox',{name:'Reason for this proposal, optional',exact:true});await reason.fill('Keep this selected scope');assert.equal(await reason.getAttribute('placeholder'),null);
  await summary.focus();await summary.press('Enter');await settle(page,watch);assert.notEqual(await aggregate.getAttribute('open'),null);
  assert((await aggregate.textContent()).includes(String(before.computed.pending.projected_count)));
  await page.screenshot({path:join(out,`inbox-expanded-${width}.png`)});
  await summary.press('Enter');assert.equal(await reason.inputValue(),'Keep this selected scope');
  const targets=await page.locator('[data-proposal-decision] button').evaluateAll(ns=>ns.map(n=>n.getBoundingClientRect().height));assert(targets.every(h=>h>=26));
  assert.equal((await axe(page)).length,0);await page.screenshot({path:join(out,`inbox-${width}.png`)});
  await page.goto(origin+'/');await settle(page,watch);
  const colour=page.getByText('Colours in deck identities',{exact:true}).locator('..');
  for(const [c,name] of [['W','white'],['U','blue'],['B','black'],['R','red'],['G','green']]){
   const meter=colour.getByRole('meter',{name:'Paper decks including '+name});await meter.waitFor();
   assert.equal(Number(await meter.getAttribute('aria-valuenow')),stats.colors[c]);assert.equal(Number(await meter.getAttribute('aria-valuemax')),stats.deck_count);
   assert.equal(await colour.getByText(name,{exact:true}).isVisible(),true);
  }
  const identities=page.getByText('Common colour identities',{exact:true}).locator('..');
  for(const i of stats.identities)assert.equal(await identities.getByText(i.identity==='C'?'Colourless':[...i.identity].join('/'),{exact:true}).isVisible(),true);
  await scrollUntilVisible(page,colour,VIEWPORTS[width]);await page.screenshot({path:join(out,`collection-colours-${width}.png`)});assert.equal((await axe(page)).length,0);
  await page.goto(origin+'/deck/2');await settle(page,watch);
  const maybe=page.getByRole('region',{name:'Maybe list'});await scrollUntilVisible(page,maybe,VIEWPORTS[width]);
  assert.equal(await maybe.getByText(`${imported.length} from Sideboard`,{exact:true}).isVisible(),true);
  const hit=maybe.locator(`[data-oracle-id="${imported[0].oracle_id}"]`).getByRole('button',{name:imported[0].name,exact:true});
  const description=await hit.getAttribute('aria-describedby');assert(description);assert.equal(await page.locator(`[id="${description}"]`).textContent(),originNote);
  assert.equal(await maybe.locator('[class*="_note_"]').filter({hasText:originNote}).count(),0);
  assert.equal(await maybe.locator('[class*="_note_"]').count(),0,'custom prose is read in details, not list rows');
  await page.screenshot({path:join(out,`maybe-${width}.png`)});await hit.focus();await hit.press('Space');await settle(page,watch);await page.getByRole('heading',{name:imported[0].name,exact:true}).waitFor();await page.getByRole('button',{name:/^Back to /}).click();await settle(page,watch);
  await page.getByRole('button',{name:'More',exact:true}).click();await page.getByRole('menuitem',{name:/slots and tags/i}).click();await settle(page,watch);
  const dialog=page.getByRole('dialog'),help=dialog.getByRole('button',{name:'How slot targets work',exact:true});await help.waitFor();
  const slots=dialog.getByText('Slots',{exact:true});
  const gap=await slots.evaluate((el,rect)=>el.getBoundingClientRect().top-rect.bottom,await help.boundingBox().then(b=>({bottom:b.y+b.height})));assert(gap>=20);
  await page.screenshot({path:join(out,`slots-${width}.png`)});assert.equal((await axe(page)).length,0);await page.keyboard.press('Escape');
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);assert.equal(watch.errors.length,0);
  assert.deepEqual(membership((await api('/api/decks/2')).state),membership(before));
  results.push({width,selected_changes_first:true,aggregate_closed_labelled:true,reason_survives_disclosure:true,decision_heights:targets,collection_colours_match_server:true,exact_identity_codes:true,import_count:imported.length,per_card_provenance_accessible:true,custom_notes_preserved:true,slot_help_gap:gap,axe:0,overflow:false,data_unchanged:true});await context.close();console.log(`Clarity ${width} passed`);
 }
 // Mixed import notes: one custom linked note and one blank must not repeat
 // the remaining shared provenance or alter sorting, assignments or membership.
 const linked=before.cards.find(c=>!c.maybe&&c.role==='card'), note=`Consider with [[${linked.name}]].`;
 for(const [card,text] of [[imported[0],note]]){
  const response=await fetch(`${origin}/api/decks/2/cards/${card.oracle_id}`,{method:'PATCH',headers:{Cookie:'uxeval_user=1',Origin:origin,'Content-Type':'application/json'},body:JSON.stringify({maybe_note:text})});assert.equal(response.status,200);
 }
 // The existing shortlist capability deliberately keeps a note for an empty
 // update. Recreate one disposable shortlist entry to set up a truly blank note.
 const blankSeed=imported[1];
 for(const [method,path,body] of [['DELETE',`/api/decks/2/cards/${blankSeed.oracle_id}`],['POST','/api/decks/2/cards',{oracle_id:blankSeed.oracle_id,maybe:true,slot_id:blankSeed.slot_id,secondary_slot_ids:blankSeed.secondary_slot_ids}]]){
  const r=await fetch(origin+path,{method,headers:{Cookie:'uxeval_user=1',Origin:origin,'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined});assert.equal(r.status,200);
 }
 const mixedBefore=(await api('/api/decks/2')).state;
 const mixedContext=await newContext(browser,{origin,vp:VIEWPORTS[1440],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),mixedPage=await mixedContext.newPage(),mixedWatch=watchPage(mixedPage);
 await mixedPage.goto(origin+'/deck/2');await settle(mixedPage,mixedWatch);
 const mixedRegion=mixedPage.getByRole('region',{name:'Maybe list'});await scrollUntilVisible(mixedPage,mixedRegion,VIEWPORTS[1440]);
 await mixedRegion.getByText(`${imported.length-2} from Sideboard`,{exact:true}).waitFor();
 assert.equal(await mixedRegion.locator('[class*="_note_"]').filter({hasText:originNote}).count(),0);
 const customRow=mixedRegion.locator(`[data-oracle-id="${imported[0].oracle_id}"]`);await customRow.getByRole('button',{name:imported[0].name,exact:true}).click();await settle(mixedPage,mixedWatch);const nested=mixedPage.getByRole('region',{name:'Maybe note',exact:true}).getByRole('button',{name:linked.name,exact:true});await nested.focus();await nested.press('Enter');await settle(mixedPage,mixedWatch);await mixedPage.getByRole('heading',{name:linked.name,exact:true}).waitFor();await mixedPage.getByRole('button',{name:/^Back to /}).click();await settle(mixedPage,mixedWatch);
 const blank=mixedRegion.locator(`[data-oracle-id="${imported[1].oracle_id}"]`);assert.equal(await blank.locator('[class*="_note_"]').count(),0);await blank.getByRole('button',{name:imported[1].name,exact:true}).focus();await mixedPage.keyboard.press('Space');await settle(mixedPage,mixedWatch);await mixedPage.getByRole('heading',{name:imported[1].name,exact:true}).waitFor();await mixedPage.getByRole('button',{name:/^Back to /}).click();await settle(mixedPage,mixedWatch);
 await mixedPage.screenshot({path:join(out,'maybe-mixed-1440.png')});assert.deepEqual(membership((await api('/api/decks/2')).state),membership(mixedBefore));assert.equal((await axe(mixedPage)).length,0);await mixedContext.close();
 // A real decision must still affect just the selected proposal, with the typed reason.
 const context=await newContext(browser,{origin,vp:VIEWPORTS[1440],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);let sent;
 page.on('request',r=>{if(r.method()==='POST'&&r.url().includes('/proposals/')&&r.url().includes('/rule'))sent={url:r.url(),body:r.postDataJSON()};});
 await page.goto(origin+`/inbox?entry=proposal-${selected.id}`);await settle(page,watch);const entry=page.locator(`a[href*="proposal-${selected.id}"]`).first();if(await entry.count())await entry.click();await settle(page,watch);
 await page.getByRole('textbox',{name:'Reason for this proposal, optional',exact:true}).fill('Selected-only scope check');await page.locator('[data-proposal-decision]').getByRole('button',{name:'Reject',exact:true}).click();await settle(page,watch);
 const after=(await api('/api/decks/2')).state;assert(!after.proposals.some(p=>p.id===selected.id));assert.deepEqual(after.proposals,before.proposals.filter(p=>p.id!==selected.id));assert.deepEqual(membership(after),membership(mixedBefore));assert.equal(after.deck.revision,before.deck.revision);assert(sent);assert.equal(sent.body.reason,'Selected-only scope check');assert.equal(sent.body.expected_revision,before.deck.revision);
 writeFileSync(join(out,'qa.json'),JSON.stringify({results,mixed_provenance:{remaining_import_count:imported.length-2,blank_row_keyboard:true,nested_custom_note_keyboard:true,raw_notes_assignments_and_order_unchanged:true},selected_only_decision:{request:sent,other_proposals_unchanged:true,membership_unchanged:true,revision_unchanged:true}},null,2)+'\n');await context.close();
}finally{await browser.close();}
