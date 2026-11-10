#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {CHECKS_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot,scrollUntilVisible} from './lib/browser.mjs';
import {formatPrice} from '../web/src/domain/price.ts';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8853',out=opt('out')??join(CHECKS_ROOT,'manual','forecast-scale');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'checks_user=1'}})).json());
const before=(await api('/api/decks/2')).state,stats=await api('/api/stats'),selected=before.proposals.find(p=>p.items.length===1);assert(selected&&before.proposals.length>1);
const browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
const checkSummary=async(page,state)=>{
 const summary=page.locator('summary').filter({hasText:`Deck impact if all ${state.proposals.length} pending proposals are accepted`});await summary.waitFor();
 const text=(await summary.textContent()).replace(/\s+/g,' ');assert(text.includes(`Cards ${state.computed.card_count} → ${state.computed.pending.projected_count}`));
 if(state.computed.cost.as_of)assert(text.includes(`Est. ${formatPrice(state.computed.cost.cents,state.computed.cost.currency)} → ${formatPrice(state.computed.pending.projected_cost.cents,state.computed.cost.currency)}`));
 if(state.computed.cost.unpriced||state.computed.pending.projected_cost.unpriced)assert(text.includes(`Unpriced ${state.computed.cost.unpriced} → ${state.computed.pending.projected_cost.unpriced}`));
 return summary;
};
try{for(const width of [1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+`/inbox?entry=proposal-${selected.id}`);await settle(page,watch);
 const summary=await checkSummary(page,before),details=summary.locator('..');assert.equal(await details.getAttribute('open'),null);
 const reason=page.getByRole('textbox',{name:'Reason for this proposal, optional',exact:true});await reason.fill('Preserve this reason while comparing the full deck forecast.');
 await shoot(page,join(out,`inbox-summary-${width}.png`),{full:false});
 await summary.focus();await summary.press('Enter');await settle(page,watch);assert.notEqual(await details.getAttribute('open'),null);assert(page.url().includes('/inbox'),'native disclosure activation cannot trigger the screen Enter shortcut');
 assert.equal(await reason.inputValue(),'Preserve this reason while comparing the full deck forecast.');await shoot(page,join(out,`inbox-expanded-${width}.png`),{full:false});
 await summary.press('Space');await settle(page,watch);assert.equal(await details.getAttribute('open'),null);assert.equal(await reason.inputValue(),'Preserve this reason while comparing the full deck forecast.');
 const changes=page.getByText('Changes in this proposal',{exact:true});assert(await changes.evaluate((el,s)=>!!(el.compareDocumentPosition(document.querySelector(s))&Node.DOCUMENT_POSITION_FOLLOWING),'details'));
 for(const item of selected.items.filter(i=>i.reason)){await page.getByText(item.reason.replace(/\[\[|\]\]/g,''),{exact:true}).waitFor();}
 assert.equal((await axe(page)).length,0);
 await page.goto(origin+'/');await settle(page,watch);const colours=page.getByText('Colours in deck identities',{exact:true}).locator('..');await scrollUntilVisible(page,colours,VIEWPORTS[width]);
 for(const [c,name] of [['W','white'],['U','blue'],['B','black'],['R','red'],['G','green']]){
  const meter=colours.getByRole('meter',{name:'Paper decks including '+name});assert.equal(Number(await meter.getAttribute('aria-valuenow')),stats.colors[c]);assert.equal(Number(await meter.getAttribute('aria-valuemax')),stats.deck_count);assert.equal(await meter.locator('..').getByText(`${stats.colors[c]} / ${stats.deck_count}`,{exact:true}).isVisible(),true);
 }
 await shoot(page,join(out,`collection-scale-${width}.png`),{full:false});assert.equal((await axe(page)).length,0);
 await page.goto(origin+'/deck/2');await settle(page,watch);await page.getByRole('button',{name:/^Review in the list/}).first().click();await settle(page,watch);
 const panel=page.getByRole('complementary',{name:'Deck panel',exact:true});assert.equal(await panel.locator('kbd').filter({hasText:/^(J|K|⇧A)$/}).count(),0,'review shortcut prose occurs only in persistent banner');
 const bannerKeys=page.locator('kbd').filter({hasText:/^J$/});assert.equal(await bannerKeys.count(),1);const units=page.locator('[data-review-unit]'),firstKey=await units.first().getAttribute('data-review-unit');
 await page.locator('body').click({position:{x:1,y:1}});await page.keyboard.press('j');await settle(page,watch);assert.equal(await page.locator('[data-review-unit][class*="_current_"]').getAttribute('data-review-unit'),await units.nth(1).getAttribute('data-review-unit'));
 await page.keyboard.press('k');await settle(page,watch);assert.equal(await page.locator('[data-review-unit][class*="_current_"]').getAttribute('data-review-unit'),firstKey);
 await shoot(page,join(out,`review-${width}.png`),{full:false});assert.equal((await axe(page)).length,0);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 assert.deepEqual((await api('/api/decks/2')).state,before);results.push({width,all_pending_scope_exact_server_values:true,selected_changes_first:true,reason_preserved:true,native_enter_space:true,visible_colour_count_denominators:true,review_shortcuts_once_and_functional:true,axe:0,deck_unchanged:true});await context.close();console.log(`Forecast and scale ${width} passed`);
}
// Verify live capability refresh after a real decision on this disposable copy.
const context=await newContext(browser,{origin,vp:VIEWPORTS[1440],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
await page.goto(origin+`/inbox?entry=proposal-${selected.id}`);await settle(page,watch);await page.getByRole('textbox',{name:'Reason for this proposal, optional',exact:true}).fill('Keep the saved deck unchanged in this fixture.');await page.locator('[data-proposal-decision]').getByRole('button',{name:'Reject',exact:true}).click();await settle(page,watch);
const after=(await api('/api/decks/2')).state;assert.equal(after.proposals.length,before.proposals.length-1);assert.deepEqual(after.cards,before.cards);assert.equal(after.deck.revision,before.deck.revision);await page.goto(origin+`/inbox?entry=proposal-${after.proposals[0].id}`);await settle(page,watch);await checkSummary(page,after);await shoot(page,join(out,'inbox-after-decision.png'),{full:false});await context.close();
writeFileSync(join(out,'qa.json'),JSON.stringify({results,post_decision_forecast_matches_capability:true,rejection_preserves_membership_and_revision:true},null,2)+'\n');
}finally{await browser.close();}
