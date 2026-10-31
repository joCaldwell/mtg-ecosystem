#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {CHECKS_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot} from './lib/browser.mjs';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8855',baseline=process.argv.includes('--baseline'),out=opt('out')??join(CHECKS_ROOT,'manual',baseline?'search-scope-baseline':'search-scope');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'checks_user=1'}})).json());
const before=(await api('/api/decks/2')).state,[a,b]=before.cards.filter(c=>!c.maybe&&c.role==='card').slice(0,2);assert(a&&b);
const browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{for(const width of opt('width')?[Number(opt('width'))]:baseline?[1440]:[1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}}),page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+'/deck/2');await settle(page,watch);const input=page.getByRole('combobox',{name:'Add a card or search',exact:true});
 await input.fill(a.name);const firstOption=page.getByRole('option').filter({has:page.getByText(a.name,{exact:true})});await firstOption.waitFor();
 let release,requestedResolve;const held=new Promise(r=>{release=r;}),requested=new Promise(r=>{requestedResolve=r;});
 await page.route('**/api/search?**',async route=>{requestedResolve();await held;await route.continue();},{times:1});
 await input.fill(b.name);await requested;
 const staleOptions=await page.locator('[role=listbox] [role=option]').count();await input.press('Enter');await settle(page,watch,{timeout:500});
 if(baseline){assert(staleOptions>0);await page.getByRole('heading',{name:a.name,exact:true}).waitFor();await shoot(page,join(out,`wrong-card-${width}.png`),{full:false});release();results.push({width,reproduced:true,input:b.name,wrong_card_opened:a.name});}
 else {
  assert.equal(staleOptions,0,'a changed query cannot retain actionable results from a previous query');assert.equal(await page.getByRole('button',{name:/^Back to /}).count(),0,'Enter during loading cannot open a stale card');
  assert.equal(await input.getAttribute('aria-activedescendant'),null);await page.getByText('Searching…',{exact:true}).waitFor();await shoot(page,join(out,`loading-${width}.png`),{full:false});assert.deepEqual(await axe(page),[]);
  release();const newOption=page.getByRole('option').filter({has:page.getByText(b.name,{exact:true})});await newOption.waitFor();await input.press('Enter');await settle(page,watch);await page.getByRole('heading',{name:b.name,exact:true}).waitFor();await page.getByRole('button',{name:/^Back to /}).click();
  // The colour-scope toggle also invalidates old options before its response.
  await input.focus();await newOption.waitFor();let releaseScope,scopeRequestResolve;const scopeHeld=new Promise(r=>{releaseScope=r;}),scopeRequested=new Promise(r=>{scopeRequestResolve=r;});
  await page.route('**/api/search?**',async route=>{scopeRequestResolve();await scopeHeld;await route.continue();},{times:1});await page.getByRole('checkbox',{name:'Search every colour',exact:true}).check();await scopeRequested;assert.equal(await page.locator('[role=listbox] [role=option]').count(),0);await input.press('Shift+Enter');assert.equal(await page.getByRole('button',{name:/^Back to /}).count(),0);releaseScope();await newOption.waitFor();
  await shoot(page,join(out,`results-${width}.png`),{full:false});assert.deepEqual(await axe(page),[]);
  await input.fill('mv<0');await page.getByText('No cards match this search.',{exact:true}).waitFor();await input.press('Enter');assert.equal(await page.getByRole('button',{name:/^Back to /}).count(),0);assert.equal(await input.getAttribute('aria-expanded'),'false');await shoot(page,join(out,`empty-${width}.png`),{full:false});assert.deepEqual(await axe(page),[]);
  assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  results.push({width,stale_query_and_colour_scope_not_actionable:true,enter_during_loading_noop:true,correct_card_after_response:true,no_matches_has_no_actionable_options:true,axe:0});
 }
 assert.deepEqual((await api('/api/decks/2')).state,before,'existing-card inspection never edits membership or assignments');await context.close();console.log(`Search scope ${width} ${baseline?'reproduced':'passed'}`);
}writeFileSync(join(out,'qa.json'),JSON.stringify({results,card_facts:'fixture API'},null,2)+'\n');}finally{await browser.close();}
