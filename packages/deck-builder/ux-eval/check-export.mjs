#!/usr/bin/env node
import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {join} from 'node:path';
import {chromium} from 'playwright-core';
import {UX_EVAL_ROOT} from './lib/paths.mjs';
import {BROWSER_ARGS,VIEWPORTS,newContext,watchPage,settle,axe,shoot} from './lib/browser.mjs';
const opt=n=>{const i=process.argv.indexOf('--'+n);return i<0?undefined:process.argv[i+1];};
const origin=opt('origin')??'http://127.0.0.1:8855',out=opt('out')??join(UX_EVAL_ROOT,'manual','export-scope');assert.equal(new URL(origin).hostname,'127.0.0.1');mkdirSync(out,{recursive:true});
const api=async path=>(await(await fetch(origin+path,{headers:{Cookie:'uxeval_user=1'}})).json());
const before=(await api('/api/decks/2')).state,maybe=before.cards.find(c=>c.maybe),main=before.cards.find(c=>!c.maybe&&c.role==='card');assert(maybe&&main);
const browser=await chromium.launch({args:BROWSER_ARGS}),results=[];
try{for(const width of [1280,1440,1920,2560]){
 const context=await newContext(browser,{origin,vp:VIEWPORTS[width],user:1,frozenNow:'2026-10-20T17:00:00Z',notes:{blocked:new Set(),imageFailures:new Set()}});await context.grantPermissions(['clipboard-read','clipboard-write'],{origin});const page=await context.newPage(),watch=watchPage(page);
 await page.goto(origin+'/deck/2');await settle(page,watch);
 const open=async()=>{await page.getByRole('button',{name:'More',exact:true}).click();await page.getByRole('menuitem',{name:'Export',exact:true}).click();await settle(page,watch);};
 await open();const dialog=page.getByRole('dialog',{name:'Export',exact:true}),field=dialog.getByRole('textbox',{name:'Exported list',exact:true}),copy=dialog.getByRole('button',{name:'Copy',exact:true});
 const check=async(query,expectedText=true)=>{await settle(page,watch);const expected=await api('/api/decks/2/export?'+query);if(expectedText){assert.equal(await field.inputValue(),expected.text);assert.equal(await copy.isEnabled(),true);await copy.click();assert.equal(await page.evaluate(()=>navigator.clipboard.readText()),expected.text);}else {assert.equal(expected.text,'');assert.equal(await field.count(),0);assert.equal(await copy.isDisabled(),true);}return {cards:expected.card_count,lines:expected.line_count};};
 const whole=await check('');await shoot(page,join(out,`whole-${width}.png`),{full:false});
 await dialog.getByRole('checkbox',{name:'Include maybe list',exact:true}).check();const withMaybe=await check('include_maybe=on');assert.equal(withMaybe.cards-whole.cards,before.cards.filter(c=>c.maybe).reduce((n,c)=>n+c.quantity,0));
 await dialog.getByRole('checkbox',{name:'Include categories',exact:true}).uncheck();await check('include_maybe=on&categories=off');
 await page.keyboard.press('Escape');const search=page.getByRole('searchbox',{name:'Search this deck',exact:true});await search.fill(maybe.name);await settle(page,watch);await open();
 assert.equal(await dialog.getByRole('checkbox',{name:'Only current filter matches',exact:true}).isChecked(),true);assert.equal(await dialog.getByRole('checkbox',{name:'Include categories',exact:true}).isChecked(),false);
 const ids=before.cards.filter(c=>c.name.toLowerCase().includes(maybe.name.toLowerCase())).map(c=>c.oracle_id).sort().join(',');assert(ids.split(',').includes(maybe.oracle_id));
 await check('categories=off&oracle_ids='+ids,false);await dialog.getByRole('checkbox',{name:'Include maybe list',exact:true}).check();const matched=await check('categories=off&include_maybe=on&oracle_ids='+ids);assert(matched.cards>=maybe.quantity);await shoot(page,join(out,`filtered-maybe-${width}.png`),{full:false});
 // Hold only this fixture's next export response. Copy must never use an old scope.
 let release;const held=new Promise(r=>{release=r;});let requestedResolve;const requested=new Promise(r=>{requestedResolve=r;});
 await page.route('**/api/decks/2/export?**',async route=>{requestedResolve();await held;await route.continue();},{times:1});
 await dialog.getByRole('checkbox',{name:'Only current filter matches',exact:true}).uncheck();await requested;assert.equal(await copy.isDisabled(),true,'previous response cannot be copied while scope changes');assert.equal(await field.count(),0);release();await check('categories=off&include_maybe=on');
 assert.equal((await axe(page)).length,0);assert.equal(watch.errors.length,0);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
 assert.deepEqual((await api('/api/decks/2')).state,before,'all export options and clipboard actions are read-only');results.push({width,whole,withMaybe,filteredMaybe:matched,clipboard_exact:true,pending_scope_copy_disabled:true,deck_unchanged:true,axe:0});await context.close();console.log(`Export scope ${width} passed`);
}writeFileSync(join(out,'qa.json'),JSON.stringify({results,card_facts:'fixture API',pending_proposals_excluded:true},null,2)+'\n');}finally{await browser.close();}
