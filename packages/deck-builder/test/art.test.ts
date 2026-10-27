import {createApp} from "../src/app.ts";
import type {Actor} from "../src/actor.ts";
import type {AddressInfo} from "node:net";
import {test} from "node:test";
import assert from "node:assert/strict";
import {creditedArt,isScryfallArtCrop,visitBackdrop} from "../src/art.ts";
import {listDecks,createDeck} from "../src/deck/service.ts";
import {applyChanges} from "../src/deck/membership.ts";
import {testDb,add,newDeck} from "./helpers.ts";
import {OWNER} from "./actors.ts";
const crop="https://cards.scryfall.io/art_crop/front/test.jpg";
test("decorative art requires a nonblank artist and a stored Scryfall crop",()=>{
 const c={oracle_id:"id-forest",name:"Forest",image_art_crop:crop,artist:" Illustrator "};
 assert.equal(creditedArt(c)?.artist,"Illustrator");
 for(const artist of [null,""," "])assert.equal(creditedArt({...c,artist}),null);
 for(const url of ["https://example.test/art.jpg","https://cards.scryfall.io/normal/front/test.jpg","https://cards.scryfall.io.evil/art_crop/test.jpg","https://cards.scryfall.io/art_crop/../large/front/test.jpg","http://cards.scryfall.io/art_crop/front/test.jpg","https://user@cards.scryfall.io/art_crop/front/test.jpg","not a url"])assert.equal(creditedArt({...c,image_art_crop:url}),null);
 assert.equal(isScryfallArtCrop(crop),true);
});
test("roster art is limited to each principal's actual commanders, with paired credits",()=>{
 const db=testDb();try{
 db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
 const other={...OWNER,user_id:2},privateId=createDeck(db,other,"Private",{});
 const id=newDeck(db);add(db,id,"id-teferi",{role:"commander"});add(db,id,"id-atraxa",{role:"commander"});add(db,id,"id-forest");
 db.prepare("UPDATE cards SET image_art_crop=?,artist='Shared illustrator'").run(crop);
 const own=listDecks(db,OWNER);assert.deepEqual(own.map(d=>d.id),[id]);assert.equal(own[0]!.commander_art.length,2);
 assert.deepEqual(listDecks(db,other).map(d=>d.id),[privateId]);assert.deepEqual(listDecks(db,other)[0]!.commander_art,[]);
 db.prepare("UPDATE cards SET artist=NULL WHERE oracle_id='id-atraxa'").run();assert.equal(listDecks(db,OWNER)[0]!.commander_art.length,1);
 applyChanges(db,OWNER,id,[{action:"cut",oracle_id:"id-teferi"}],{kind:"edit"});assert.deepEqual(listDecks(db,OWNER)[0]!.commander_art,[]);
 }finally{db.close();}
});
test("anonymous visit backdrop contains shared card facts only, never unknown credits or retired art",()=>{
 const db=testDb();try{
 assert.equal(visitBackdrop(db),null);
 db.prepare("UPDATE cards SET image_art_crop=?,artist='Environment illustrator' WHERE name='Forest'").run(crop);
 assert.deepEqual(visitBackdrop(db),{oracle_id:"id-forest",name:"Forest",image_art_crop:crop,artist:"Environment illustrator"});
 db.prepare("UPDATE cards SET retired=1 WHERE name='Forest'").run();assert.equal(visitBackdrop(db),null);
 }finally{db.close();}
});

test("bootstrap art matrix: owner/other UI and anonymous receive shared facts; agent principals cannot read UI account info",async()=>{
 const db=testDb(),saved=process.env.DECKBUILDER_PUBLIC_ORIGIN;
 db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
 db.prepare("UPDATE cards SET image_art_crop=?,artist='Environment illustrator' WHERE name='Forest'").run(crop);
 let principal:Actor|null=null;
 const app=createApp(db,{mode:"hosted",resolveUiActor:()=>principal});
 await new Promise<void>(r=>app.server.listen(0,"127.0.0.1",r));
 const origin=`http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;process.env.DECKBUILDER_PUBLIC_ORIGIN=origin;
 try {
  for(const actor of [OWNER,{...OWNER,via:"agent",label:"agent"},{...OWNER,user_id:2},{...OWNER,user_id:2,via:"agent",label:"agent"},null] as (Actor|null)[]){
   principal=actor;const res=await fetch(`${origin}/api/session`);
   if(actor?.via==="agent"){assert.equal(res.status,403);assert.equal((await res.json() as any).backdrop,undefined);continue;}
   assert.equal(res.status,200);
   const session=await res.json() as any;assert.deepEqual(session.backdrop,{oracle_id:"id-forest",name:"Forest",image_art_crop:crop,artist:"Environment illustrator"});
   assert.equal(session.user?.id??null,actor?.user_id??null);
  }
 }finally{await app.close();db.close();if(saved===undefined)delete process.env.DECKBUILDER_PUBLIC_ORIGIN;else process.env.DECKBUILDER_PUBLIC_ORIGIN=saved;}
});
