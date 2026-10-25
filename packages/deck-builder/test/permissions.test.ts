// Phase 1 of the §4 matrix: all decks are private. Visibility/suggestions
// extend this dimension in Phase 4a; anonymous access is tested at both doors.
import { randomUUID } from "node:crypto";
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Database } from "../src/sqlite.ts";
import { type Actor, PermissionError, isDeckOwner, requireDeckOwner, requireSelf } from "../src/actor.ts";
import * as decks from "../src/deck/service.ts";
import * as brief from "../src/deck/brief.ts";
import * as checks from "../src/deck/checks.ts";
import * as fork from "../src/deck/fork.ts";
import * as interop from "../src/deck/interop.ts";
import * as log from "../src/deck/log.ts";
import * as membership from "../src/deck/membership.ts";
import * as power from "../src/deck/power.ts";
import * as proposals from "../src/deck/proposals.ts";
import * as sourcing from "../src/deck/sourcing.ts";
import * as documents from "../src/documents.ts";
import * as memory from "../src/memory.ts";
import * as activity from "../src/activity.ts";
import * as focus from "../src/focus.ts";
import * as stats from "../src/stats.ts";
import * as state from "../src/state.ts";
import * as playtest from "../src/playtest/service.ts";
import * as search from "../src/search/index.ts";
import * as tokens from "../src/tokens.ts";
import { getSetting, setSetting } from "../src/db.ts";
import { TOOLS, callTool } from "../src/mcp/tools.ts";
import { testDb } from "./helpers.ts";
import { OWNER, agent } from "./actors.ts";

const modules = { decks, brief, checks, fork, interop, log, membership, power, proposals, sourcing, documents, memory, activity, focus, stats, state, playtest, search, tokens, settings: {getSetting,setSetting} };
const functions = Object.fromEntries(Object.values(modules).flatMap(m => Object.entries(m).filter(([, v]) => typeof v === "function"))) as Record<string, (...args: any[]) => any>;
type Classification = "read" | "write" | "owner-only" | "suggest" | "public-read";
type Target = { id: number; ui: Actor; ag: Actor; slot: number; tag: number; engine: number; memory: number; proposal: number; item: number; edit: number; log: number; session: ReturnType<typeof playtest.startPlaytest>; category: decks.DeckCategory; token: number };
type Fixture = ReturnType<typeof fixture>;
type Case = { kind: Classification; run: (db: Database, actor: Actor, t: Target, f: Fixture) => unknown; agentBlocked?: "all" | "paper"; archived?: boolean; global?: boolean };
const deckCase = (kind: Classification, args: (t: Target, f: Fixture) => any[] = () => [], more: Partial<Case> = {}) => ({ kind, args, ...more });
const defs = {
  createSlot: deckCase("write", () => ["New purpose"]), createTag: deckCase("write", () => ["New tag"]),
  defineSlots: deckCase("write", () => [[{ name: "New purpose" }]]), deleteDeck: deckCase("write"),
  deleteSlot: deckCase("write", t => [t.slot]), deleteTag: deckCase("write", t => [t.tag]),
  fileCards: deckCase("write", () => [[{ oracle_id: "id-forest", add_tags: ["New tag"] }]]),
  findDeckCard: deckCase("read", () => ["id-forest"]), findSlotByName: deckCase("read", () => ["Utility"]),
  getDeck: deckCase("read"), listSlots: deckCase("read"), listTags: deckCase("read"),
  organizeCard: deckCase("write", () => ["id-forest", { tagIds: [] }]), recomputeIdentity: deckCase("write"),
  renameDeck: deckCase("write", () => ["Renamed"]), renameTag: deckCase("write", t => [t.tag, "Renamed tag"]),
  requireDeck: deckCase("read"), requireDirectCardEdit: deckCase("write", () => [], { agentBlocked: "paper" }),
  requireEditableDeck: deckCase("write"), requireRevision: deckCase("read"),
  requireSlotByName: deckCase("read", () => ["Utility"]), secondarySlotsOf: deckCase("read", () => ["id-forest"]),
  setDeckCategory: deckCase("owner-only", () => ["theorycrafted"], { agentBlocked: "all", archived: true }),
  setDeckParent: deckCase("owner-only", () => [null], { agentBlocked: "all" }),
  shortlistCard: deckCase("write", () => ["id-solring", "candidate"]), unshortlistCard: deckCase("write", () => ["id-solring"]),
  updateSlot: deckCase("write", t => [t.slot, { targetMin: 1 }]), validateSlotAssignments: deckCase("read", () => [null, []]),
  getBrief: deckCase("read"), removeEngine: deckCase("owner-only", t => [t.engine], { agentBlocked: "all" }),
  removeEngineByName: deckCase("owner-only", () => ["Engine"], { agentBlocked: "all" }),
  setEngine: deckCase("owner-only", () => ["New engine", "", []], { agentBlocked: "all" }),
  updateBrief: deckCase("owner-only", () => [{ thesis: "Updated" }], { agentBlocked: "all" }),
  checkDeck: deckCase("read"), exportDeck: deckCase("read"), diffImport: deckCase("read", () => ["7 Forest"]),
  applyImport: deckCase("owner-only", () => ["7 Forest"], { agentBlocked: "all" }),
  fillFromList: deckCase("write", () => ["7 Forest"], { agentBlocked: "paper" }),
  getPower: deckCase("read"), getPowerTarget: deckCase("read"), gameChangerFacts: deckCase("read"),
  recordPowerAssessment: deckCase("write", () => [{ profile: {}, revision: 1, evidence: "list", confidence: "tentative", notes: "List evidence" }]),
  setPowerTarget: deckCase("owner-only", () => [null], { agentBlocked: "all" }),
  writePowerTarget: deckCase("owner-only", () => [null], { agentBlocked: "all" }),
  getPlaytest: deckCase("read", () => [], { agentBlocked: "all" }),
  startPlaytest: deckCase("owner-only", t => [{ mode: "practice", replace: true, session_id: t.session.session.id, expected_version: 0 }], { agentBlocked: "all", archived: true }),
  changePlaytest: deckCase("owner-only", t => [{ session_id: t.session.session.id, expected_version: 0, action: { type: "keep" } }], { agentBlocked: "all", archived: true }),
  savePlaytestUi: deckCase("owner-only", t => [{ session_id: t.session.session.id, expected_version: 0, ui: { selected: [] } }], { agentBlocked: "all", archived: true }),
  saveObservation: deckCase("owner-only", t => [{ session_id: t.session.session.id, request_id: randomUUID(), text: "Tested preference", oracle_id: null }], { agentBlocked: "all" }),
} as const;
const cases: Record<string, Case> = Object.fromEntries(Object.entries(defs).map(([name, def]) => [name, {
  ...def, run: (db: Database, a: Actor, t: Target, f: Fixture) => functions[name](db, a, t.id, ...def.args(t, f)),
}]));
function scoped(name: string, kind: Classification, run: Case["run"], more: Partial<Case> = {}) { cases[name] = { kind, run, ...more }; }
scoped("applyChanges", "write", (db,a,t) => membership.applyChanges(db,a,t.id,[{ action:"add",oracle_id:"id-solring" }],{kind:"edit"}), {agentBlocked:"paper"});
scoped("setQuantity", "write", (db,a,t) => membership.setQuantity(db,a,t.id,"id-forest",8), {agentBlocked:"paper"});
scoped("undo", "owner-only", (db,a,t) => membership.undo(db,a,t.log), {agentBlocked:"all"});
scoped("forkDeck", "write", (db,a,t) => fork.forkDeck(db,a,t.id,"New fork"), {archived:true});
scoped("compareDecks", "read", (db,a,t,f) => fork.compareDecks(db,a,t.id,f.otherOwn(a,t).id));
scoped("copyPowerTarget", "write", (db,a,t,f) => power.copyPowerTarget(db,a,t.id,f.otherOwn(a,t).id), {archived:true});
scoped("copyDeckMemories", "write", (db,a,t,f) => memory.copyDeckMemories(db,a,t.id,f.otherOwn(a,t).id), {archived:true});
scoped("compareDeckSources", "read", (db,a,t,f) => sourcing.compareDeckSources(db,a,t.id,[f.otherOwn(a,t).id]));
scoped("recordOwnership", "write", (db,a,t) => sourcing.recordOwnership(db,a,t.id,[{oracle_id:"id-forest",status:"unknown",basis:"Explicit evidence"}]));
scoped("cardHistory", "read", (db,a,t) => log.cardHistory(db,a,t.id,"id-forest"));
scoped("cardHistories", "read", (db,a,t) => log.cardHistories(db,a,t.id,["id-forest"]));
scoped("listLog", "read", (db,a,t) => log.listLog(db,a,{deckId:t.id}));
scoped("getLogEntry", "read", (db,a,t) => log.getLogEntry(db,a,t.log));
scoped("markReviewed", "write", (db,a,t) => log.markReviewed(db,a,{through:t.log}), {archived:true});
scoped("writeLog", "write", (db,a,t) => log.writeLog(db,a,{deck_id:t.id,kind:"edit"}), {archived:true});
scoped("createProposal", "write", (db,a,t) => proposals.createProposal(db,a,t.id,{items:[{action:"cut",oracle_id:"id-forest",reason:"Preference"}]}));
scoped("getProposal", "read", (db,a,t) => proposals.getProposal(db,a,t.proposal));
scoped("listProposals", "read", (db,a,t) => proposals.listProposals(db,a,{deckId:t.id}));
scoped("pendingItemIds", "read", (db,a,t) => proposals.pendingItemIds(db,a,t.proposal));
scoped("acceptItems", "owner-only", (db,a,t) => proposals.acceptItems(db,a,[t.item]), {agentBlocked:"all"});
scoped("acceptProposals", "owner-only", (db,a,t) => proposals.acceptProposals(db,a,t.id,[{proposal_id:t.proposal}],1), {agentBlocked:"all"});
scoped("rejectItems", "owner-only", (db,a,t) => proposals.rejectItems(db,a,[t.item]), {agentBlocked:"all"});
scoped("withdrawProposal", "write", (db,a,t) => proposals.withdrawProposal(db,a,t.proposal,"Withdraw"));
scoped("amendProposal", "write", (db,a,t) => proposals.amendProposal(db,a,t.proposal,{items:[{action:"cut",oracle_id:"id-forest",reason:"Changed"}]}));
scoped("getDocumentEdit", "read", (db,a,t) => documents.getDocumentEdit(db,a,t.edit));
scoped("listDocumentEdits", "read", (db,a,t) => documents.listDocumentEdits(db,a,{deckId:t.id}));
scoped("proposeDocumentEdit", "suggest", (db,a,t) => documents.proposeDocumentEdit(db,a,t.id,"constraints",{content:"New constraint"},"Preference"));
scoped("proposeDocumentGroup", "suggest", (db,a,t) => documents.proposeDocumentGroup(db,a,t.id,[{kind:"constraints",payload:{content:"New constraint"},reason:"Preference"}]));
scoped("ruleDocumentEdit", "owner-only", (db,a,t) => documents.ruleDocumentEdit(db,a,t.edit,"reject"), {agentBlocked:"all"});
scoped("withdrawDocumentEdit", "write", (db,a,t) => documents.withdrawDocumentEdit(db,a,t.edit,"Withdraw"));
scoped("getMemory", "read", (db,a,t) => memory.getMemory(db,a,t.memory));
scoped("listMemories", "read", (db,a,t) => memory.listMemories(db,a,{deckId:t.id}));
scoped("loadScope", "read", (db,a,t) => memory.loadScope(db,a,"deck",t.id));
scoped("memoriesForCards", "read", (db,a,t) => memory.memoriesForCards(db,a,["id-forest"],t.id));
scoped("remember", "write", (db,a,t) => memory.remember(db,a,{scope:"deck",deck_id:t.id,text:"A new deck preference"}));
scoped("reviseMemory", "write", (db,a,t) => memory.reviseMemory(db,a,t.memory,{action:"retire"}));
scoped("setFocus", "write", (db,a,t) => focus.setFocus(db,a,t.id), {archived:true});
scoped("deckState", "read", (db,a,t) => state.deckState(db,a,t.id));
scoped("search", "read", (db,a,t) => search.search(db,a,"t:land",{deckId:t.id}));
scoped("recordActivity", "write", (db,a,t) => activity.recordActivity(db,a,"matrix",t.id,"Visible write"), {archived:true});

// These touch only the principal, so isolation assertions below inspect their
// results instead of supplying another user's deck id.
for (const [name,kind] of Object.entries({getSetting:"read",setSetting:"write",createDeck:"write",listDecks:"read",getProfile:"read",setProfile:"owner-only",getBudgets:"read",setBudgets:"write",unreviewedCount:"read",getFocus:"read",playerStats:"read",listActivity:"read",ownershipEvidence:"read",createToken:"owner-only",listTokens:"owner-only",revokeToken:"owner-only"}) as [string, Classification][]) {
  cases[name] = { kind, global:true, run:()=>{} };
}
// Pure calculations and operator/authentication utilities have no tenant data.
const publicFunctions = ["copyLimit","similarTag","validateVocabName","isDoctorPair","parseArchidektList","mapBracket","parseTarget","profileLines","targetText","estimateTokens","tableCard","SearchError","verifyToken","migrateLegacyToken"];
for(const name of publicFunctions) cases[name] = {kind:"public-read",global:true,run:()=>{}};

test("every exported capability is explicitly classified; new exports fail until reviewed", () => {
  assert.deepEqual(Object.keys(cases).sort(), Object.keys(functions).sort());
  for(const [name,c] of Object.entries(cases)) {
    assert.ok(["read","write","owner-only","suggest","public-read"].includes(c.kind),name);
    if(c.kind !== "public-read") assert.match(functions[name].toString(), /\bactor\b/,name);
  }
});

function fixture() {
  const db=testDb();
  db.exec("INSERT INTO users(id,handle,display_name) VALUES (2,'second','Second')");
  const users=[OWNER,{...OWNER,user_id:2}];
  const targets:Target[]=[];
  for(const ui of users) {
    documents.setProfile(db,ui,`Private profile ${ui.user_id}`);
    memory.remember(db,agent("test-agent",ui.user_id),{scope:"global",text:`Private global preference ${ui.user_id}`});
    for(const category of ["paper","theorycrafted"] as const) {
      const ag=agent("test-agent",ui.user_id), id=decks.createDeck(db,ui,category,{category});
      const slot=decks.createSlot(db,ui,id,"Utility"), tag=decks.createTag(db,ui,id,"Have");
      const logId=membership.applyChanges(db,ui,id,[{action:"add",oracle_id:"id-forest",quantity:7}],{kind:"edit"})!;
      decks.organizeCard(db,ui,id,"id-forest",{tagIds:[tag]});
      decks.shortlistCard(db,ui,id,"id-solring");
      brief.setEngine(db,ui,id,"Engine","",[]);
      const engine=brief.getBrief(db,ui,id).engines[0].id;
      const mem=memory.remember(db,ag,{scope:"deck",deck_id:id,text:`Private deck preference ${id}`,cards:["id-forest"]}).memory.id;
      const proposal=proposals.createProposal(db,ag,id,{items:[{action:"add",oracle_id:"id-solring",reason:"Preference"}]});
      const item=proposals.pendingItemIds(db,ui,proposal)[0];
      const edit=documents.proposeDocumentEdit(db,ag,id,"thesis",{content:"Proposed thesis"},"Preference").edit_id!;
      const session=playtest.startPlaytest(db,ui,id,{mode:"practice"});
      const token=tokens.createToken(db,ui,"Matrix client").credential.id;
      targets.push({id,ui,ag,slot,tag,engine,memory:mem,proposal,item,edit,log:logId,session,category,token});
    }
    focus.setFocus(db,ui,targets.find(t=>t.ui.user_id===ui.user_id)!.id);
    memory.setBudgets(db,ui,{global:1000+ui.user_id});
  }
  return {db,users,targets,otherOwn:(a:Actor,t:Target)=>targets.find(x=>x.ui.user_id===a.user_id&&x.id!==t.id)!};
}

for(const [name,c] of Object.entries(cases).filter(([,c])=>!c.global)) {
  for(const category of ["paper","theorycrafted","legacy"] as const) {
    test(`${name}: private ${category}, owner-ui/agent and other-ui/agent`,()=>{
      for(const via of ["ui","agent"] as const) {
        const f=fixture(),t=f.targets.find(t=>t.ui.user_id===1&&t.category===(category==="legacy"?"theorycrafted":category))!;
        try {
          if(category==="legacy") decks.setDeckCategory(f.db,t.ui,t.id,"legacy");
          const other={user_id:2,via,label:via==="ui"?"ui":"test-agent"} as Actor;
          const foreign=()=>c.run(f.db,other,t,f);
          if(c.kind==="read") assert.throws(foreign,(e:any)=>e.status===404,`${name} foreign read`);
          else assert.throws(foreign,PermissionError,`${name} foreign write`);
          const own=via==="ui"?t.ui:t.ag;
          const blocked=(via==="agent"&&(c.agentBlocked==="all"||(category==="paper"&&c.agentBlocked==="paper"))) ||
            (category==="legacy"&&c.kind!=="read"&&!c.archived);
          if(blocked) assert.throws(()=>c.run(f.db,own,t,f),PermissionError,`${name} preserves lifecycle/human gate`);
          else assert.doesNotThrow(()=>c.run(f.db,own,t,f),`${name} own ${via}`);
          assert.deepEqual(f.db.prepare("PRAGMA foreign_key_check").all(),[]);
        } finally {f.db.close();}
      }
    });
  }
}

test("all per-user feeds, searches, settings and credentials exclude the other principal",()=>{
  const f=fixture(),{db}=f;
  try {
    for(const ui of f.users) {
      const own=f.targets.filter(t=>t.ui.user_id===ui.user_id), other=f.targets.filter(t=>t.ui.user_id!==ui.user_id);
      const ag=agent("test-agent",ui.user_id);
      assert.deepEqual(decks.listDecks(db,ag).map(d=>d.id).sort(),own.map(t=>t.id).sort());
      assert.equal(documents.getProfile(db,ag).body_md,`Private profile ${ui.user_id}`);
      assert.throws(()=>documents.setProfile(db,ag,"agent direct"),PermissionError);
      assert.equal(memory.getBudgets(db,ag).global,1000+ui.user_id);
      setSetting(db,ui,"private",String(ui.user_id)); assert.equal(getSetting(db,ag,"private"),String(ui.user_id));
      assert.equal(focus.getFocus(db,ag)!.deck_id,own[0].id);
      assert.equal(stats.playerStats(db,ag).deck_count,1);
      for(const q of [{},{query:"private"},{scope:"global" as const},{cardId:"id-forest"}]) {
        const rows=memory.listMemories(db,ag,q);assert.ok(rows.length); assert.ok(rows.every(m=>m.user_id===ui.user_id));
      }
      const linked=memory.memoriesForCards(db,ag,["id-forest"],own[0].id).get("id-forest")!;
      assert.ok(linked.every(m=>m.user_id===ui.user_id));
      const found=memory.remember(db,ag,{scope:"global",text:"A different private global preference"});
      assert.ok(found.similar.every(m=>m.user_id===ui.user_id));
      const proposalsForUser=proposals.listProposals(db,ag);
      assert.equal(proposalsForUser.length,2);assert.ok(proposalsForUser.every(p=>own.some(t=>t.id===p.deck_id)));
      documents.proposeDocumentEdit(db,ag,null,"profile",{content:"Pending profile"},"Preference");
      const edits=documents.listDocumentEdits(db,ag);
      assert.equal(edits.length,3);assert.ok(edits.every(e=>e.author_user_id===ui.user_id));
      for(const e of edits) assert.equal(e.author_display,`${ui.user_id===1?"owner":"second"} via test-agent`);
      assert.equal(log.unreviewedCount(db,ag),1);
      assert.ok(log.listLog(db,ag).every(e=>own.some(t=>t.id===e.deck_id)));
      assert.ok(activity.listActivity(db,ag).every(e=>e.user_id===ui.user_id&&e.actor_user_id===ui.user_id&&e.actor_via==="agent"));
      assert.ok(sourcing.ownershipEvidence(db,ag,"id-forest").every(e=>e.source_deck_id!=null&&own.some(t=>t.id===e.source_deck_id)));
      assert.throws(()=>decks.setDeckParent(db,ui,own[0].id,other[0].id),PermissionError);
      assert.throws(()=>decks.createDeck(db,ag,"Bad parent",{parentId:other[0].id}),PermissionError);
      assert.throws(()=>memory.remember(db,ag,{scope:"global",text:"Bad source",source_log_id:other[0].log}),PermissionError);
      assert.throws(()=>log.markReviewed(db,ag,{ids:[own[0].log,other[0].log]}),PermissionError);
      assert.equal(log.unreviewedCount(db,ag),1,"mixed review IDs cannot partially write");
      const credential=tokens.createToken(db,ui,"test-agent");
      assert.equal(tokens.verifyToken(db,credential.token)!.user_id,ui.user_id);
      assert.ok(tokens.listTokens(db,ui).every(t=>t.user_id===ui.user_id));
      assert.throws(()=>tokens.listTokens(db,ag),PermissionError);
      assert.throws(()=>tokens.createToken(db,ag,"forbidden"),PermissionError);
      tokens.revokeToken(db,ui,credential.credential.id); assert.equal(tokens.verifyToken(db,credential.token),null);
    }
    // Even malformed ancestry cannot disclose another principal's history.
    const [a,,b]=f.targets;
    db.prepare("UPDATE decks SET parent_id = ? WHERE id = ?").run(b.id,a.id);
    assert.ok(log.cardHistory(db,a.ag,a.id,"id-forest").every(r=>r.log_id!==b.log));
    assert.equal(decks.getDeck(db,a.ag,a.id).deck.parent,null);
    assert.equal(isDeckOwner(db,a.ag,a.id),true);assert.equal(isDeckOwner(db,b.ag,a.id),false);
    assert.throws(()=>requireDeckOwner(db,b.ag,a.id),PermissionError);assert.throws(()=>requireSelf(a.ag,2),PermissionError);
  } finally {db.close();}
});

type ToolCase={kind:Classification;args:(t:Target,f:Fixture)=>Record<string,unknown>; global?:boolean; paperBlocked?:boolean};
const toolCases:Record<string,ToolCase>={
  start_session:{kind:"read",global:true,args:()=>({})}, player_stats:{kind:"read",global:true,args:()=>({})},
  create_deck:{kind:"write",global:true,args:()=>({name:"New tool deck"})},
  propose_profile_edit:{kind:"suggest",global:true,args:()=>({text:"A new profile",reason:"Preference"})},
  get_deck:{kind:"read",args:t=>({deck:t.id})}, compare_decks:{kind:"read",args:(t,f)=>({deck:t.id,other:f.otherOwn(t.ag,t).id})},
  compare_deck_sources:{kind:"read",args:(t,f)=>({deck:t.id,sources:[f.otherOwn(t.ag,t).id]})},
  search_cards:{kind:"read",args:t=>({deck:t.id,query:"t:land",include_my_decks:true})},
  get_cards:{kind:"read",args:t=>({deck:t.id,cards:["Forest"],include_my_decks:true})},
  list_rulings:{kind:"read",args:t=>({deck:t.id})}, search_memories:{kind:"read",args:t=>({deck:t.id,query:"private"})},
  list_proposals:{kind:"read",args:t=>({deck:t.id,include_history:true})}, export_deck:{kind:"read",args:t=>({deck:t.id})},
  rename_deck:{kind:"write",args:t=>({deck:t.id,name:"Renamed by tool"})},delete_deck:{kind:"write",args:t=>({deck:t.id})},
  fork_deck:{kind:"write",args:t=>({deck:t.id,name:"Tool fork"})},
  edit_cards:{kind:"write",paperBlocked:true,args:t=>({deck:t.id,add:[{card:"Sol Ring"}]})},
  define_slots:{kind:"write",args:t=>({deck:t.id,slots:[{name:"New purpose"}]})},
  file_cards:{kind:"write",args:t=>({deck:t.id,filings:[{card:"Forest",add_tags:["New tag"]}]})},
  record_ownership:{kind:"write",args:t=>({deck:t.id,cards:[{card:"Forest",status:"unknown",basis:"Preference"}]})},
  shortlist_cards:{kind:"write",args:t=>({deck:t.id,add:[{card:"Sol Ring",note:"Consider"}]})},
  remember:{kind:"write",args:t=>({deck:t.id,scope:"deck",text:"A new tool preference"})},
  revise_memory:{kind:"write",args:t=>({id:t.memory,action:"retire"})},
  mark_rulings_reviewed:{kind:"write",args:t=>({through:t.log})},
  propose_changes:{kind:"suggest",args:t=>({deck:t.id,items:[{action:"cut",card:"Forest",reason:"Preference"}]})},
  propose_brief_edit:{kind:"suggest",args:t=>({deck:t.id,kind:"constraints",text:"A new constraint",reason:"Preference"})},
  propose_power_target:{kind:"suggest",args:t=>({deck:t.id,target:null,reason:"Preference"})},
  assess_deck_power:{kind:"write",args:t=>({deck:t.id,assessment:{profile:{},revision:1,evidence:"list",confidence:"tentative",notes:"List evidence"}})},
  propose_document_changes:{kind:"suggest",args:t=>({deck:t.id,changes:[{kind:"constraints",text:"New constraint",reason:"Preference"}]})},
  revise_proposal:{kind:"write",args:t=>({type:"cards",id:t.proposal,action:"withdraw",reason:"Preference"})},
};
test("every MCP tool has an explicit permission classification",()=>assert.deepEqual(Object.keys(toolCases).sort(),TOOLS.map(t=>t.name).sort()));
for(const [name,c] of Object.entries(toolCases)) {
  test(`MCP ${name}: both users, Paper and Theorycrafted, own and other data`,()=>{
    for(const userId of [1,2]) for(const category of ["paper","theorycrafted"]) {
      const f=fixture(), t=f.targets.find(t=>t.ui.user_id===userId&&t.category===category)!,other=f.targets.find(t=>t.ui.user_id!==userId&&t.category===category)!;
      try {
        if(!c.global) assert.throws(()=>callTool({db:f.db,actor:t.ag},name,c.args(other,f)),c.kind==="read"?(e:any)=>e.status===404:PermissionError,`${name} other deck`);
        const run=()=>callTool({db:f.db,actor:t.ag},name,c.args(t,f));
        if(c.paperBlocked&&category==="paper") assert.throws(run,PermissionError);
        else { const result=run();assert.ok(result.text.length>0); }
      } finally {f.db.close();}
    }
  });
}

test("MCP roster, focus, identity, global memory/FTS, card context and include_my_decks belong to the bearer",()=>{
  const f=fixture();
  try {
    for(const ui of f.users) {
      const ag=agent("test-agent",ui.user_id), own=f.targets.filter(t=>t.ui.user_id===ui.user_id),other=f.targets.filter(t=>t.ui.user_id!==ui.user_id);
      const session=callTool({db:f.db,actor:ag},"start_session").text;
      assert.match(session,new RegExp(`# Identity\\n${ui.user_id===1?"owner — Owner":"second — Second"}`));
      assert.ok(session.includes(`Private profile ${ui.user_id}`));assert.ok(!session.includes(`Private profile ${other[0].ui.user_id}`));
      assert.ok(!session.includes(`Private global preference ${other[0].ui.user_id}`));
      const mem=callTool({db:f.db,actor:ag},"search_memories",{query:"private"}).text;
      for(const t of other) assert.ok(!mem.includes(`Private deck preference ${t.id}`));
      // Same names exist in both accounts; name resolution chooses this account.
      const byName=callTool({db:f.db,actor:ag},"get_deck",{deck:decks.requireDeck(f.db,ag,own[0].id).name,sections:["memories"]}).text;
      assert.ok(byName.includes(`Private deck preference ${own[0].id}`));
      assert.ok(!byName.includes(`Private deck preference ${other[0].id}`));
      decks.renameDeck(f.db,other[0].ui,other[0].id,`Other private list ${ui.user_id}`);
      for(const name of ["get_cards","search_cards"]) {
        const args=name==="get_cards"?{cards:["Forest"]}:{query:"t:land"};
        const text=callTool({db:f.db,actor:ag},name,{...args,include_my_decks:true}).text;
        assert.ok(!text.includes(`Other private list ${ui.user_id}`));
        assert.ok(text.includes(own[0].id===1?"paper":"Other private list 1"));
      }
    }
  } finally {f.db.close();}
});

// The HTTP registry is discovered too: adding a route without a matrix row fails.
import { createApp } from "../src/app.ts";
import type { AddressInfo } from "node:net";
type HttpCase = { method:string; path:(t:Target,f:Fixture)=>string; kind:Classification; body?:(t:Target)=>unknown; global?:boolean; ownStatus?:number };
const httpCases:HttpCase[]=[];
const http=(method:string,path:HttpCase["path"],kind:Classification,body?:HttpCase["body"],more:Partial<HttpCase>={})=>httpCases.push({method,path,kind,body,...more});
const deckPath=(t:Target)=>`/api/decks/${t.id}`;
http("GET",()=>"/api/health","public-read",undefined,{global:true});
http("GET",()=>"/api/decks","read",undefined,{global:true});
http("POST",()=>"/api/decks","write",()=>({name:"New HTTP deck"}),{global:true});
http("GET",deckPath,"read");
http("PATCH",deckPath,"write",()=>({name:"Renamed HTTP deck"}));
http("DELETE",deckPath,"write");
http("POST",t=>`${deckPath(t)}/fork`,"write",()=>({name:"HTTP fork"}));
http("GET",(t,f)=>`${deckPath(t)}/compare?other=${f.otherOwn(t.ui,t).id}`,"read");
http("GET",()=>"/api/cards/id-forest/table","public-read",undefined,{global:true});
http("GET",t=>`${deckPath(t)}/playtest`,"read");
http("POST",t=>`${deckPath(t)}/playtest`,"owner-only",t=>({mode:"practice",replace:true,session_id:t.session.session.id,expected_version:0}));
http("POST",t=>`${deckPath(t)}/playtest/actions`,"owner-only",t=>({session_id:t.session.session.id,expected_version:0,action:{type:"keep"}}));
http("PATCH",t=>`${deckPath(t)}/playtest/ui`,"owner-only",t=>({session_id:t.session.session.id,expected_version:0,ui:{selected:[]}}));
http("POST",t=>`${deckPath(t)}/playtest/observations`,"owner-only",t=>({session_id:t.session.session.id,request_id:randomUUID(),text:"Played preference",oracle_id:null}));
http("POST",t=>`${deckPath(t)}/cards`,"write",()=>({oracle_id:"id-solring"}));
http("PATCH",t=>`${deckPath(t)}/cards/id-forest`,"write",()=>({quantity:8}));
http("DELETE",t=>`${deckPath(t)}/cards/id-forest`,"write");
http("GET",t=>`${deckPath(t)}/cards/id-forest/history`,"read");
http("POST",t=>`${deckPath(t)}/slots`,"write",()=>({name:"New purpose"}));
http("PATCH",t=>`${deckPath(t)}/slots/${t.slot}`,"write",()=>({name:"Changed purpose"}));
http("DELETE",t=>`${deckPath(t)}/slots/${t.slot}`,"write");
http("POST",t=>`${deckPath(t)}/tags`,"write",()=>({name:"New tag"}));
http("PATCH",t=>`${deckPath(t)}/tags/${t.tag}`,"write",()=>({name:"Changed tag"}));
http("DELETE",t=>`${deckPath(t)}/tags/${t.tag}`,"write");
http("GET",()=>"/api/inbox","read",undefined,{global:true});
http("GET",()=>"/api/proposal-history","read",undefined,{global:true});
http("POST",t=>`${deckPath(t)}/items/rule`,"owner-only",t=>({item_ids:[t.item],outcome:"reject"}));
http("POST",t=>`${deckPath(t)}/proposals/rule`,"owner-only",t=>({expected_revision:1,proposals:[{proposal_id:t.proposal}]}));
http("POST",t=>`${deckPath(t)}/proposals/${t.proposal}/rule`,"owner-only",()=>({outcome:"reject"}));
http("POST",t=>`/api/document-edits/${t.edit}/rule`,"owner-only",()=>({outcome:"reject"}));
http("POST",t=>`${deckPath(t)}/log/${t.log}/undo`,"owner-only");
http("GET",t=>`/api/log?deck=${t.id}`,"read");
http("GET",t=>`${deckPath(t)}/power`,"read");
http("PUT",t=>`${deckPath(t)}/power/target`,"owner-only",()=>({target:null}));
http("POST",t=>`${deckPath(t)}/power/assessments`,"write",()=>({profile:{},revision:1,evidence:"list",confidence:"tentative",notes:"List evidence"}));
http("PUT",t=>`${deckPath(t)}/brief`,"owner-only",()=>({thesis:"New thesis"}));
http("POST",t=>`${deckPath(t)}/engines`,"owner-only",()=>({name:"New engine",pieces:[]}));
http("DELETE",t=>`${deckPath(t)}/engines/${t.engine}`,"owner-only");
http("GET",()=>"/api/profile","read",undefined,{global:true});
http("PUT",()=>"/api/profile","owner-only",()=>({body_md:"New HTTP profile"}),{global:true});
http("GET",t=>`/api/memories?deck=${t.id}`,"read");
http("POST",()=>"/api/memories","write",t=>({scope:"deck",deck_id:t.id,text:"New HTTP memory"}));
http("PATCH",t=>`/api/memories/${t.memory}`,"write",()=>({action:"retire"}));
http("GET",t=>`/api/memories/${t.memory}`,"read");
http("GET",()=>"/api/activity","read",undefined,{global:true});
http("GET",()=>"/api/stats","read",undefined,{global:true});
http("GET",()=>"/api/focus","read",undefined,{global:true});
http("GET",()=>"/api/settings","owner-only",undefined,{global:true});
http("PUT",()=>"/api/settings","owner-only",()=>({budgets:{global:1200}}),{global:true});
http("POST",()=>"/api/settings/tokens","owner-only",()=>({label:"HTTP client"}),{global:true});
http("DELETE",(t,f)=>`/api/settings/tokens/${t.token}`,"owner-only");
http("POST",()=>"/api/cards/refresh","owner-only",undefined,{global:true,ownStatus:503});
http("GET",t=>`${deckPath(t)}/export`,"read");
http("POST",t=>`${deckPath(t)}/import/preview`,"read",()=>({text:"7 Forest"}));
http("POST",t=>`${deckPath(t)}/import`,"owner-only",()=>({text:"7 Forest"}));
http("GET",()=>"/api/resolve?name=Forest","public-read",undefined,{global:true});
http("GET",t=>`/api/search?q=t:land&deck=${t.id}`,"read");
http("PUT",()=>"/api/focus","write",t=>({deck_id:t.id}));

async function listening(app:ReturnType<typeof createApp>) {
  await new Promise<void>(resolve=>app.server.listen(0,"127.0.0.1",resolve));
  return `http://127.0.0.1:${(app.server.address() as AddressInfo).port}`;
}
function send(base:string,c:HttpCase,t:Target,f:Fixture) {
  const body=c.body?.(t);
  return fetch(base+c.path(t,f),{method:c.method,headers:body?{"Content-Type":"application/json"}:{},...(body?{body:JSON.stringify(body)}:{})});
}
test("every HTTP API route is classified and reachable only with the right principal",async()=>{
  const f=fixture(); let principal:Actor|null=OWNER;
  const app=createApp(f.db,{resolveUiActor:()=>principal}),base=await listening(app);
  try {
    const routeIds=(c:HttpCase)=>app.routes.map((r,i)=>r.method===c.method&&r.pattern.test(new URL(c.path(f.targets[0],f),base).pathname)?i:-1).filter(i=>i>=0);
    assert.ok(httpCases.every(c=>routeIds(c).length===1));
    assert.deepEqual([...new Set(httpCases.flatMap(routeIds))].sort((a,b)=>a-b),app.routes.map((_,i)=>i));
    for(const category of ["paper","theorycrafted"]) for(const c of httpCases) {
      const own=f.targets.find(t=>t.ui.user_id===1&&t.category===category)!;
      // Each mutation runs in a savepoint and rolls back, so every row sees
      // the same meaningful setup without hiding the request's validation.
      f.db.exec("SAVEPOINT http_matrix");
      try {
        principal=OWNER;
        const other=f.targets.find(t=>t.ui.user_id===2&&t.category===category)!;
        if(!c.global) {
          const res=await send(base,c,other,f);
          assert.equal(res.status,c.kind==="read"?404:403,`${c.method} ${c.path(other,f)} cross-user: ${await res.text()}`);
        }
        const res=await send(base,c,own,f);
        assert.equal(res.status,c.ownStatus??200,`${c.method} ${c.path(own,f)} own: ${await res.text()}`);
        principal=null;
        const anonymous=await send(base,c,own,f);
        assert.equal(anonymous.status,c.path(own,f)==="/api/health"?200:401,`${c.method} anonymous`);
        await anonymous.arrayBuffer();
      } finally { f.db.exec("ROLLBACK TO http_matrix; RELEASE http_matrix"); }
    }
  } finally {await app.close();f.db.close();}
});

test("hosted UI APIs await sessions; health stays public; no bearer may select UI data",async()=>{
  const f=fixture(),saved=process.env.DECKBUILDER_PUBLIC_ORIGIN;
  const app=createApp(f.db,{mode:"hosted"}),base=await listening(app);
  process.env.DECKBUILDER_PUBLIC_ORIGIN=base;
  try {
    const token=tokens.createToken(f.db,OWNER,"Hosted agent").token;
    assert.equal((await fetch(base+"/api/health")).status,200);
    for(const path of ["/api/decks","/api/settings","/api/events"]) assert.equal((await fetch(base+path,{headers:{Authorization:`Bearer ${token}`}})).status,401);
    assert.equal((await fetch(base+"/mcp")).status,401);
  } finally {await app.close();f.db.close();if(saved===undefined)delete process.env.DECKBUILDER_PUBLIC_ORIGIN;else process.env.DECKBUILDER_PUBLIC_ORIGIN=saved;}
});

import { Client, StreamableHTTPClientTransport } from "@modelcontextprotocol/client";
test("per-user tokens and SSE isolate both doors; URL labels cannot become UI; revoke takes effect on the next request",async()=>{
  const f=fixture(),app=createApp(f.db,{resolveUiActor:req=>f.users.find(u=>u.user_id===Number(req.headers["x-matrix-user"]??1))??null}),base=await listening(app);
  const clients:Client[]=[];const readers:ReadableStreamDefaultReader<Uint8Array>[]=[];
  try {
    for(const ui of f.users) {
      const stream=await fetch(base+"/api/events",{headers:{"x-matrix-user":String(ui.user_id)}});
      const reader=stream.body!.getReader(); readers.push(reader);
      assert.equal(new TextDecoder().decode((await reader.read()).value),'data: {"kind":"connected"}\n\n');
    }
    const credentials=f.users.map(ui=>tokens.createToken(f.db,ui,"test-agent"));
    for(const credential of credentials) {
      const client=new Client({name:"same-client",version:"1.0.0"}); clients.push(client);
      await client.connect(new StreamableHTTPClientTransport(new URL(base+"/mcp/owner"),{requestInit:{headers:{Authorization:`Bearer ${credential.token}`}}}));
    }
    const a=f.targets[0],b=f.targets[2];
    const foreign=await clients[0].callTool({name:"get_deck",arguments:{deck:b.id}});
    assert.equal(foreign.isError,true);assert.match(JSON.stringify(foreign.content),/There is no deck/);
    const forbidden=await clients[0].callTool({name:"remember",arguments:{deck:b.id,scope:"deck",text:"Cross user"}});
    assert.equal(forbidden.isError,true);assert.match(JSON.stringify(forbidden.content),/Only the deck owner/);
    const saved=await clients[0].callTool({name:"remember",arguments:{scope:"global",text:"From authenticated user A"}});
    assert.ok(!saved.isError);
    assert.equal(new TextDecoder().decode((await readers[0].read()).value),'data: {"kind":"remember"}\n\n');
    const authored=memory.listMemories(f.db,a.ui,{scope:"global"})[0];
    assert.equal(authored.author_via,"agent");assert.equal(authored.owner_touched,false);assert.equal(authored.author_label,"owner");
    assert.equal(memory.listMemories(f.db,b.ui,{query:"authenticated"}).length,0);
    const uiProposal=proposals.createProposal(f.db,a.ui,a.id,{items:[{action:"cut",oracle_id:"id-forest",reason:"Human proposal"}]});
    const impersonation=await clients[0].callTool({name:"revise_proposal",arguments:{type:"cards",id:uiProposal,action:"withdraw",reason:"Pretend human"}});
    assert.equal(impersonation.isError,true);assert.match(JSON.stringify(impersonation.content),/only their own pending/);
    await fetch(base+"/api/memories",{method:"POST",headers:{"Content-Type":"application/json","x-matrix-user":"2"},body:JSON.stringify({scope:"global",text:"From user B UI"})});
    assert.equal(new TextDecoder().decode((await readers[1].read()).value),'data: {"kind":"memories"}\n\n',"B receives no A event");
    app.notify(1,{kind:"marker",deck_id:a.id});
    assert.equal(new TextDecoder().decode((await readers[0].read()).value),`data: {"kind":"marker","deck_id":${a.id}}\n\n`,"A receives no B event");
    const draft=await clients[0].callTool({name:"propose_brief_edit",arguments:{deck:a.id,kind:"constraints",text:"Credential-owned draft",reason:"Preference"}});
    assert.ok(!draft.isError);
    const edit=documents.listDocumentEdits(f.db,a.ui,{deckId:a.id}).find(e=>e.kind==="constraints")!;
    const extra=tokens.createToken(f.db,a.ui,"test-agent");
    const rival=new Client({name:"same-client",version:"1.0.0"});clients.push(rival);
    await rival.connect(new StreamableHTTPClientTransport(new URL(base+"/mcp/owner"),{requestInit:{headers:{Authorization:`Bearer ${extra.token}`}}}));
    const denied=await rival.callTool({name:"revise_proposal",arguments:{type:"document",id:edit.id,action:"withdraw",reason:"Impersonating the same URL label"}});
    assert.equal(denied.isError,true);assert.match(JSON.stringify(denied.content),/only their own pending/);
    const relabelled=new Client({name:"same-client",version:"1.0.0"});clients.push(relabelled);
    await relabelled.connect(new StreamableHTTPClientTransport(new URL(base+"/mcp/different-hint"),{requestInit:{headers:{Authorization:`Bearer ${credentials[0].token}`}}}));
    const permitted=await relabelled.callTool({name:"revise_proposal",arguments:{type:"document",id:edit.id,action:"withdraw",reason:"Same authenticated credential, different attribution"}});
    assert.ok(!permitted.isError,"changing a label neither grants nor removes draft ownership");
    tokens.revokeToken(f.db,a.ui,credentials[0].credential.id);
    await assert.rejects(()=>clients[0].callTool({name:"start_session",arguments:{}}));
    const session=await clients[1].callTool({name:"start_session",arguments:{}});
    assert.ok(!session.isError);assert.match(JSON.stringify(session.content),/second — Second/);
  } finally {for(const reader of readers)await reader.cancel();for(const c of clients)await c.close();await app.close();f.db.close();}
});
