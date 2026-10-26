// Phase 1 of the §4 matrix: all decks are private. Visibility/suggestions
// extend this dimension in Phase 4a; anonymous access is tested at both doors.
import { globSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
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
import * as prices from "../src/prices.ts";
import { PKG_ROOT, getSetting, setSetting } from "../src/db.ts";
import { TOOLS, callTool } from "../src/mcp/tools.ts";
import { testDb } from "./helpers.ts";
import { OWNER, agent } from "./actors.ts";

// Executable front doors are tested through their tool/route registries below;
// operator modules intentionally open files, spawn children or download shared
// card facts. They are not tenant capabilities. Everything else, including new
// modules, renderers, cards and prices, is discovered and must be classified.
const excludedModules: Record<string,string> = {
  "logging.ts":"transport observability only; secrecy and request isolation covered by logging.test.ts",
  "server.ts":"server entry point", "cli.ts":"operator CLI entry point", "mcp/stdio.ts":"stdio entry point",
  "app.ts":"HTTP registry tested below", "mcp/http.ts":"authenticated MCP transport tested below", "mcp/tools.ts":"tool registry tested below",
  "sqlite.ts":"SQLite constructor/backup primitives", "backups.ts":"operator backup/restore",
  "ingest.ts":"shared card ingest", "jobs.ts":"operator scheduler", "json-array.ts":"stream parser",
  "auth.ts":"authentication boundary, including registration transactions, exercised in hosted-auth.test.ts",
  "http-body.ts":"bounded transport parser, exercised through both HTTP doors",
  "cimd-transport.ts":"pinned DNS transport security, exercised in hosted-auth.test.ts",
  "auth-database.ts":"shared connection reservation and native dialect, covered by hosted security regressions",
  "oauth-policy.ts":"signed authorization identity boundary, covered by hosted security regressions",
  "auth-config.ts":"pinned library configuration, not a tenant capability",
  "ratelimit.ts":"HTTP boundary limiter, exercised in hosted-auth.test.ts",
  "process-lock.ts":"operator process locks", "refresh.ts":"operator refresh", "scryfall.ts":"shared bulk downloads",
};
const functions: Record<string,any> = {};
const sourceRoot=join(PKG_ROOT,"src");
for(const file of [...globSync("**/*.ts",{cwd:sourceRoot})].sort()) {
  if(file.endsWith(".d.ts") || excludedModules[file]) continue; // declaration files have no runtime exports
  const module=await import(pathToFileURL(join(sourceRoot,file)).href);
  for(const [name,fn] of Object.entries(module)) if(typeof fn === "function") {
    assert.ok(!functions[name] || functions[name] === fn,`${file} duplicates ${name}; qualify the matrix keys before adding it`);
    functions[name]=fn;
  }
}
const notFound = (error: any) => error.status === 404;
/** A deterministic 10-character base62 slug per fixture deck. */
const slugOf = (t: { id: number }) => `S${String(t.id).padStart(9, "0")}`;
type Classification = "read" | "write" | "owner-only" | "suggest" | "public-read";
type Target = { id: number; ui: Actor; ag: Actor; slot: number; tag: number; engine: number; memory: number; proposal: number; item: number; edit: number; log: number; session: ReturnType<typeof playtest.startPlaytest>; category: decks.DeckCategory; token: number };
type Fixture = ReturnType<typeof fixture>;
type Case = { kind: Classification; run: (db: Database, actor: Actor, t: Target, f: Fixture) => unknown; agentBlocked?: "all" | "paper"; archived?: boolean; global?: boolean; probe?: GlobalProbe };
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

// Sharing (§18). Visibility and publishing are the owner's, in the app; the
// slug-addressed reads are classified public-read and get their own matrix below.
scoped("setVisibility","owner-only",(db,a,t)=>functions.setVisibility(db,a,t.id,"unlisted"),{agentBlocked:"all",archived:true});
scoped("setPublishDecisions","owner-only",(db,a,t)=>functions.setPublishDecisions(db,a,t.id,true),{agentBlocked:"all",archived:true});
scoped("hideLogEntry","owner-only",(db,a,t)=>log.hideLogEntry(db,a,t.log,true),{agentBlocked:"all",archived:true});
scoped("sharingOf","read",(db,a,t)=>functions.sharingOf(db,a,t.id),{archived:true});
scoped("requireViewableDeck","public-read",(db,a,t)=>decks.requireViewableDeck(db,a,slugOf(t)),{archived:true});
scoped("publicDeckState","public-read",(db,a,t)=>state.publicDeckState(db,a,slugOf(t)),{archived:true});
scoped("exportPublicDeck","public-read",(db,a,t)=>interop.exportPublicDeck(db,a,slugOf(t)),{archived:true});
scoped("renderDeck","read",(db,a,t)=>functions.renderDeck(db,a,t.id));
scoped("afterWrite","read",(db,a,t)=>functions.afterWrite(db,a,t.id));
scoped("isDeckOwner","read",(db,a,t)=>{const own=functions.isDeckOwner(db,a,t.id);assert.equal(own,a?.user_id===t.ui.user_id);if(!own)throw Object.assign(new Error("unreadable"),{status:404});return own;});
scoped("requireDeckOwner","write",(db,a,t)=>functions.requireDeckOwner(db,a,t.id),{archived:true});

type GlobalProbe = (invoke:any,f:Fixture,a:Actor,t:Target)=>unknown|Promise<unknown>;
function globalCase(name:string,kind:Classification,probe:GlobalProbe) {
  cases[name]={kind,global:true,probe,run:async(_db,a,t,f)=>{
    let calls=0;
    const invoke=new Proxy(functions[name],{
      apply(fn,self,args){calls++;return Reflect.apply(fn,self,args);},
      construct(fn,args){calls++;return Reflect.construct(fn,args);},
    });
    const result=await probe(invoke,f,a,t);
    assert.ok(calls>0,`${name}: its global probe must actually invoke the export`);
    return result;
  }};
}

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
    activity.recordActivity(db,agent("test-agent",ui.user_id),"fixture",targets.find(t=>t.ui.user_id===ui.user_id)!.id,`Private activity ${ui.user_id}`);
  }
  // Every deck has a slug, as if once shared; visibility alone decides who reads it (§18.2).
  for(const t of targets) db.prepare("UPDATE decks SET slug=? WHERE id=?").run(slugOf(t),t.id);
  return {db,users,targets,otherOwn:(a:Actor|null,t:Target)=>targets.find(x=>x.ui.user_id===(a?.user_id??t.ui.user_id)&&x.id!==t.id)!};
}

for(const [name,c] of Object.entries(cases).filter(([,c])=>!c.global)) {
  for(const category of ["paper","theorycrafted","archived"] as const) {
    test(`${name}: private ${category}, owner-ui/agent and other-ui/agent`,()=>{
      for(const via of ["ui","agent"] as const) {
        const f=fixture(),t=f.targets.find(t=>t.ui.user_id===1&&t.category===(category==="archived"?"theorycrafted":category))!;
        try {
          if(category==="archived") decks.setDeckCategory(f.db,t.ui,t.id,"archived");
          const other={user_id:2,via,label:via==="ui"?"ui":"test-agent"} as Actor;
          const foreign=()=>c.run(f.db,other,t,f);
          assert.throws(foreign,notFound,`${name} foreign target`);
          const own=via==="ui"?t.ui:t.ag;
          const blocked=(via==="agent"&&(c.agentBlocked==="all"||(category==="paper"&&c.agentBlocked==="paper"))) ||
            (category==="archived"&&c.kind!=="read"&&!c.archived);
          if(blocked) assert.throws(()=>c.run(f.db,own,t,f),PermissionError,`${name} preserves lifecycle/human gate`);
          else assert.doesNotThrow(()=>c.run(f.db,own,t,f),`${name} own ${via}`);
          assert.deepEqual(f.db.prepare("PRAGMA foreign_key_check").all(),[]);
        } finally {f.db.close();}
      }
    });
  }
}

// ---------- Phase 4a: viewer and anonymous rows (spec §18.3, plan §4) ----------
// Visibility never opens an id-addressed capability. For every capability,
// lifecycle and visibility (including public with suggestions on, which phase
// 5 will use), another user's UI and agent and an anonymous caller (a null
// viewer) all meet the same 404 or 401, and nothing anywhere is written.
const VISIBILITY_ROWS = [["private",0],["unlisted",0],["public",0],["public",1]] as const;
const refused = (e: any) => e instanceof Error && [401, 404].includes((e as any).status);
function share(db: Database, t: Target, visibility: string, suggestions = 0, category: string = t.category) {
  db.prepare("UPDATE decks SET visibility=?, accept_suggestions=?, category=? WHERE id=?").run(visibility, suggestions, category, t.id);
}
const viewers = (t: Target): Array<[string, Actor | null]> => [
  ["other-ui", { user_id: 2, via: "ui", label: "ui" }], ["other-agent", agent("test-agent", 2)], ["anonymous", null],
].filter(([, a]) => a === null || (a as Actor).user_id !== t.ui.user_id) as Array<[string, Actor | null]>;
for(const [name,c] of Object.entries(cases).filter(([,c])=>!c.global && c.kind!=="public-read")) {
  test(`${name}: other-ui, other-agent and anonymous are refused on private, unlisted and public decks`,()=>{
    const f=fixture();
    try {
      for(const t of f.targets.filter(t=>t.ui.user_id===1)) for(const category of [t.category,"archived"]) for(const [visibility,suggestions] of VISIBILITY_ROWS) {
        share(f.db,t,visibility,suggestions,category);
        for(const [who,viewer] of viewers(t)) {
          const before=personalSnapshot(f.db);
          assert.throws(()=>c.run(f.db,viewer as Actor,t,f),refused,`${name} ${who} on ${visibility}${suggestions?"+suggestions":""} ${category}`);
          assert.deepEqual(personalSnapshot(f.db),before,`${name} ${who} wrote nothing`);
        }
      }
    } finally {f.db.close();}
  });
}

/** The exact shape a viewer receives (plan §4 PublicDeckState); anything new fails here first. */
const PUBLIC_KEYS = {
  top: ["brief","cards","checks","computed","deck","engines","power","price","slots","tags"],
  deck: ["category","color_identity","commander","name","owner","parent","slug","visibility"],
  power: ["latest_owner_assessment","target"],
  price: ["as_of","currency","source","store","total_cents","unpriced"],
  decision: ["at","by","cards","kind","reason","summary"],
};
const PRIVATE_TEXT = ["Private deck preference","Private activity","Private profile","Private global","Proposed thesis","test-agent","Matrix client","Preference","Shortlist note"];
function assertPublicShape(shared: state.PublicDeckState, published: boolean) {
  assert.deepEqual(Object.keys(shared).sort(),[...PUBLIC_KEYS.top,...(published?["decisions"]:[])].sort());
  assert.deepEqual(Object.keys(shared.deck).sort(),PUBLIC_KEYS.deck);
  assert.deepEqual(Object.keys(shared.deck.owner).sort(),["display_name","handle"]);
  assert.deepEqual(Object.keys(shared.power).sort(),PUBLIC_KEYS.power);
  assert.deepEqual(Object.keys(shared.price).sort(),PUBLIC_KEYS.price);
  assert.deepEqual(Object.keys(shared.brief).sort(),["constraints_md","thesis"]);
  for(const d of shared.decisions ?? []) { assert.deepEqual(Object.keys(d).sort(),PUBLIC_KEYS.decision); assert.ok(["owner","agent"].includes(d.by)); }
  assert.ok(shared.cards.every(c=>c.maybe_note===""),"shortlist notes stay private");
  assert.deepEqual(shared.tags,[],"tags are sourcing and ownership notes");assert.ok(shared.cards.every(c=>c.tag_ids.length===0));
  assert.ok(!shared.checks.some(f=>f.key.startsWith("power_")),"power findings read assessments that may be an agent's");
  const cost=shared.computed.cost;
  assert.deepEqual(shared.computed.pending,{adds:0,cuts:0,projected_count:shared.computed.card_count,by_slot:{},
    projected_cost:cost,cost_delta:{...cost,cents:0,unpriced:0}},"pending proposals leave no trace");
  const text=JSON.stringify(shared);
  for(const secret of PRIVATE_TEXT) assert.ok(!text.includes(secret),`public state leaks "${secret}"`);
  for(const key of ["user_id","author","author_label","author_display","author_via","author_user_id","id","revision","proposal_id","document_edit_id","memories","proposals","document_edits","log"])
    assert.ok(!new RegExp(`"${key}":`).test(JSON.stringify({...shared,cards:[],slots:[],tags:[]})),`public state carries ${key}`);
}
test("public-read capabilities: owner, other and anonymous across lifecycle × visibility get exactly PublicDeckState or 404",()=>{
  const f=fixture();
  try {
    for(const t of f.targets.filter(t=>t.ui.user_id===1)) {
      // Material a viewer must never see, beside what they may: an agent's log
      // entry and shortlist note, an owner power target and assessment.
      decks.shortlistCard(f.db,t.ui,t.id,"id-solring","Shortlist note");
      if(t.category==="theorycrafted") membership.applyChanges(f.db,t.ag,t.id,[{action:"add",oracle_id:"id-counterspell"}],{kind:"edit"});
      power.setPowerTarget(f.db,t.ui,t.id,{profile:{},game_changers_max:2,notes:"Owner target"});
      power.recordPowerAssessment(f.db,t.ag,t.id,{profile:{},revision:decks.requireDeck(f.db,t.ui,t.id).revision,evidence:"list",confidence:"tentative",notes:"Agent guess"});
      const hidden=membership.applyChanges(f.db,t.ui,t.id,[{action:"add",oracle_id:"id-llanowar"}],{kind:"edit",summary:"Hidden entry"})!;
      log.hideLogEntry(f.db,t.ui,hidden,true);
      for(const category of [t.category,"archived"]) for(const [visibility,suggestions] of VISIBILITY_ROWS) for(const published of [0,1]) {
        share(f.db,t,visibility,suggestions,category);
        f.db.prepare("UPDATE decks SET publish_decisions=? WHERE id=?").run(published,t.id);
        const actors:Array<[string,Actor|null]>=[["owner-ui",t.ui],["owner-agent",t.ag],...viewers(t)];
        for(const [who,viewer] of actors) {
          const owner=viewer?.user_id===1, readable=owner||visibility!=="private", label=`${who} ${visibility} ${category} published=${published}`;
          const before=personalSnapshot(f.db);
          if(!readable) {
            for(const run of [()=>state.publicDeckState(f.db,viewer,slugOf(t)),()=>interop.exportPublicDeck(f.db,viewer,slugOf(t)),()=>decks.requireViewableDeck(f.db,viewer,slugOf(t))])
              assert.throws(run,notFound,label);
          } else {
            const shared=state.publicDeckState(f.db,viewer,slugOf(t));
            assertPublicShape(shared,!!published);
            assert.equal(shared.deck.visibility,visibility);assert.equal(shared.deck.owner.handle,"owner");
            assert.equal(shared.power.target!.notes,"Owner target");assert.equal(shared.power.latest_owner_assessment,null,"an agent's assessment is not the owner's");
            if(published) {
              assert.ok(shared.decisions!.length>0);
              // The hidden entry is the only one that touched Llanowar Elves.
              assert.ok(!shared.decisions!.some(d=>d.cards.some(c=>c.oracle_id==="id-llanowar")||/Llanowar/.test(d.summary)),"hidden entries are omitted");
              assert.ok(!shared.decisions!.some(d=>d.summary==="Hidden entry"),"summaries are the app's, not stored free text");
              if(t.category==="theorycrafted") assert.ok(shared.decisions!.some(d=>d.by==="agent"),"agent entries say only 'agent'");
            }
            assert.equal(decks.requireViewableDeck(f.db,viewer,slugOf(t)).role,owner?"owner":"viewer");
            assert.match(interop.exportPublicDeck(f.db,viewer,slugOf(t)).text,/Forest/);
          }
          assert.deepEqual(personalSnapshot(f.db),before,`${label} wrote nothing`);
        }
      }
    }
    assert.throws(()=>state.publicDeckState(f.db,null,"NoSuchSlug"),notFound);
  } finally {f.db.close();}
});

test("cross-owner fork by slug copies the list, slots, brief and engines only, and keeps its lineage",()=>{
  const f=fixture();
  try {
    const parent=f.targets.find(t=>t.ui.user_id===1&&t.category==="paper")!, second=f.users[1], secondAgent=agent("test-agent",2);
    decks.shortlistCard(f.db,parent.ui,parent.id,"id-solring","Shortlist note");
    power.setPowerTarget(f.db,parent.ui,parent.id,{profile:{},notes:"Owner target"});
    sourcing.recordOwnership(f.db,parent.ui,parent.id,[{oracle_id:"id-forest",status:"owned",basis:"Owner evidence"}]);
    assert.throws(()=>fork.forkDeck(f.db,second,{slug:slugOf(parent)},"Too early"),notFound,"a private deck cannot be forked by another user");
    assert.throws(()=>fork.forkDeck(f.db,null as unknown as Actor,{slug:slugOf(parent)},"Anonymous"),refused);
    share(f.db,parent,"unlisted");
    for(const [who,actor] of [["ui",second],["agent",secondAgent]] as const) {
      const id=fork.forkDeck(f.db,actor,{slug:slugOf(parent)},`Second ${who} fork`);
      const view=decks.getDeck(f.db,actor,id), original=decks.getDeck(f.db,parent.ui,parent.id);
      const row=decks.requireDeck(f.db,actor,id);
      assert.equal(row.user_id,2);assert.equal(row.category,"theorycrafted");assert.equal(row.visibility,"private");
      assert.deepEqual(view.deck.upstream,{owner_handle:"owner",slug:slugOf(parent),name:original.deck.name});
      assert.equal(view.deck.parent,null,"the parent is not one of the forker's decks");
      assert.equal(row.parent_id,null,"the other user's numeric id is never stored on the fork");
      assert.ok(!JSON.stringify(exportAccount(f.db,{user_id:2,via:"ui",label:"ui"}).tables.decks).includes(`"parent_id":${parent.id}`),"nor exported");
      assert.deepEqual(view.cards.map(c=>[c.name,c.quantity,c.maybe]).sort(),original.cards.map(c=>[c.name,c.quantity,c.maybe]).sort());
      assert.ok(view.cards.every(c=>c.maybe_note===""));
      assert.deepEqual(view.slots.map(s=>s.name),original.slots.map(s=>s.name));
      assert.ok(original.tags.some(t=>t.name==="Have"),"the parent has an ownership tag");
      assert.deepEqual(view.tags,[],"tags are the owner's sourcing and ownership notes (review S4)");
      assert.ok(view.cards.every(c=>c.tag_ids.length===0));
      assert.deepEqual(brief.getBrief(f.db,actor,id).engines.map(e=>e.name),["Engine"]);
      assert.equal(memory.listMemories(f.db,actor,{deckId:id}).length,0,"a second user's fork contains no memories");
      assert.equal(f.db.prepare("SELECT COUNT(*) n FROM memories WHERE deck_id=?").get(id)!.n,0);
      assert.equal(f.db.prepare("SELECT COUNT(*) n FROM ownership_evidence WHERE user_id=2 AND source_deck_id=?").get(id)!.n,0);
      assert.equal(power.getPower(f.db,actor,id).target,null);assert.equal(power.getPower(f.db,actor,id).history.length,0);
      assert.deepEqual(log.listLog(f.db,actor,{deckId:id}).map(e=>e.summary),[`Forked from owner/${original.deck.name}`]);
      assert.equal(proposals.listProposals(f.db,actor,{deckId:id}).length,0);
      assert.ok(log.cardHistory(f.db,actor,id,"id-forest").every(r=>r.from_deck==null),"card history does not cross the owner boundary");
      assert.equal(fork.compareDecks(f.db,actor,id).b.slug,slugOf(parent));
    }
    const [forked]=decks.listDecks(f.db,second).filter(d=>d.name==="Second ui fork");
    share(f.db,parent,"private");
    assert.throws(()=>fork.compareDecks(f.db,second,forked.id),{status:404,message:/no longer read/});
    assert.ok(decks.getDeck(f.db,second,forked.id).deck.upstream,"lineage survives the parent going private");
    f.db.prepare("UPDATE decks SET visibility='public' WHERE id=?").run(forked.id);
    const forkSlug=functions.setVisibility(f.db,second,forked.id,"public").slug;
    assert.equal(state.publicDeckState(f.db,null,forkSlug).deck.parent,null,"viewers are not told about a private parent (review S1)");
    decks.deleteDeck(f.db,parent.ui,parent.id);
    assert.deepEqual(decks.getDeck(f.db,second,forked.id).deck.upstream!.owner_handle,"owner","the forker's lineage survives the parent's deletion");
  } finally {f.db.close();}
});

test("published decisions: an undo of a hidden entry is hidden too, and no summary quotes a proposal or an agent",()=>{
  const f=fixture();
  try {
    const t=f.targets.find(t=>t.ui.user_id===1&&t.category==="theorycrafted")!;
    share(f.db,t,"public");f.db.prepare("UPDATE decks SET publish_decisions=1 WHERE id=?").run(t.id);
    const added=membership.applyChanges(f.db,t.ui,t.id,[{action:"add",oracle_id:"id-llanowar"}],{kind:"edit"})!;
    membership.undo(f.db,t.ui,added);log.hideLogEntry(f.db,t.ui,added,true);
    // A brief decision's stored summary embeds the proposal's reason; an agent may name itself in one.
    const edit=documents.proposeDocumentEdit(f.db,t.ag,t.id,"constraints",{content:"No infinite combos"},"Proposed by test-agent because Secret reason").edit_id!;
    documents.ruleDocumentEdit(f.db,t.ui,edit,"accept","Owner agrees");
    membership.applyChanges(f.db,t.ag,t.id,[{action:"add",oracle_id:"id-counterspell"}],{kind:"edit",summary:"test-agent wrote this summary"});
    const decisions=state.publicDeckState(f.db,null,slugOf(t)).decisions!;
    assert.ok(!decisions.some(d=>d.cards.some(c=>c.oracle_id==="id-llanowar")),"neither the hidden entry nor its undo");
    const text=JSON.stringify(decisions);
    for(const secret of ["Secret reason","test-agent","proposed because"]) assert.ok(!text.includes(secret),secret);
    assert.ok(decisions.some(d=>d.summary==="Accepted a proposed change to the brief"&&d.reason==="Owner agrees"));
    assert.ok(decisions.some(d=>d.summary==="Added Counterspell"&&d.by==="agent"));
  } finally {f.db.close();}
});

test("a fork's parent shows its link only while public, its name while unlisted, nothing once private (review S1)",()=>{
  const f=fixture();
  try {
    const parent=f.targets.find(t=>t.ui.user_id===1&&t.category==="paper")!, second=f.users[1];
    share(f.db,parent,"unlisted");
    const id=fork.forkDeck(f.db,second,{slug:slugOf(parent)},"Second's public fork");
    const forkSlug=functions.setVisibility(f.db,second,id,"public").slug;
    const name=decks.requireDeck(f.db,parent.ui,parent.id).name;
    const seen=()=>[state.publicDeckState(f.db,null,forkSlug).deck.parent,functions.publicProfile(f.db,null,"second").decks[0].parent];
    for(const p of seen()) assert.deepEqual(p,{owner_handle:"owner",slug:null,name},"an unlisted parent's link is its owner's to give");
    share(f.db,parent,"public");
    for(const p of seen()) assert.deepEqual(p,{owner_handle:"owner",slug:slugOf(parent),name});
    share(f.db,parent,"private");
    for(const p of seen()) assert.equal(p,null,"a private parent is not named");
    assert.ok(decks.getDeck(f.db,second,id).deck.upstream,"the forker's own deck keeps the snapshot");
    decks.deleteDeck(f.db,parent.ui,parent.id);
    for(const p of seen()) assert.equal(p,null);
  } finally {f.db.close();}
});

test("public checks carry no power findings from an agent's assessment (review S2)",()=>{
  const f=fixture();
  try {
    const t=f.targets.find(t=>t.ui.user_id===1&&t.category==="theorycrafted")!;
    power.setPowerTarget(f.db,t.ui,t.id,{profile:{intent:"social"},game_changers_max:0});
    power.recordPowerAssessment(f.db,t.ag,t.id,{profile:{intent:"competitive"},revision:decks.requireDeck(f.db,t.ui,t.id).revision,evidence:"list",confidence:"tentative",notes:"Agent guess"});
    assert.ok(checks.checkDeck(f.db,t.ui,t.id).some(f=>f.key==="power_target_gap"),"the owner sees the gap");
    share(f.db,t,"public");
    const shared=state.publicDeckState(f.db,null,slugOf(t));
    assert.ok(!shared.checks.some(f=>f.key.startsWith("power_")));
    assert.ok(!JSON.stringify(shared).includes("cEDH"));
    assert.ok(!callTool({db:f.db,actor:agent("other",2)},"get_deck",{deck:slugOf(t)}).text.includes("cEDH"));
  } finally {f.db.close();}
});

test("a shared deck is priced for its viewer: their own choice, else USD/lowest, never the owner's (plan §4, D18)",()=>{
  const f=fixture();
  try {
    const t=f.targets.find(t=>t.ui.user_id===1&&t.category==="paper")!;
    f.db.prepare("DELETE FROM card_prices WHERE oracle_id='id-forest'").run();
    f.db.prepare("INSERT INTO card_prices(oracle_id,store,currency,cents,set_code,collector_number,finish) VALUES ('id-forest','tcgplayer','USD',10,'tst','1','nonfoil'),('id-forest','cardmarket','EUR',7,'tst','1','nonfoil')").run();
    share(f.db,t,"public");
    prices.setPriceSource(f.db,t.ui,{currency:"EUR",store:"lowest"});
    const anonymous=state.publicDeckState(f.db,null,slugOf(t)).price;
    assert.deepEqual([anonymous.currency,anonymous.total_cents],["USD",70],"anonymous: USD/lowest, 7 Forests at 10¢");
    const second=f.users[1];
    assert.equal(state.publicDeckState(f.db,second,slugOf(t)).price.currency,"USD","another viewer's default, not the owner's EUR");
    prices.setPriceSource(f.db,second,{currency:"EUR",store:"cardmarket"});
    const eur=state.publicDeckState(f.db,second,slugOf(t));
    assert.deepEqual([eur.price.currency,eur.price.store,eur.price.total_cents],["EUR","cardmarket",49]);
    assert.ok(eur.cards.filter(c=>c.oracle_id==="id-forest").every(c=>c.currency==="EUR"&&c.cents===7),"card prices follow the viewer too");
    assert.equal(state.publicDeckState(f.db,t.ui,slugOf(t)).price.currency,"EUR","the owner previews in their own choice");
    // 1b's price capabilities with no caller: the anonymous default, and no write.
    assert.deepEqual(prices.getPriceSource(f.db,null),{currency:"USD",store:"lowest"});
    assert.throws(()=>prices.setPriceSource(f.db,null,{currency:"EUR",store:"lowest"}),{status:401});
  } finally {f.db.close();}
});

test("a profile exists only while its person has a public deck (review S3)",()=>{
  const f=fixture();
  try {
    const t=f.targets.find(t=>t.ui.user_id===2)!;
    assert.throws(()=>functions.publicProfile(f.db,null,"second"),notFound);
    share(f.db,t,"unlisted");
    assert.throws(()=>functions.publicProfile(f.db,null,"second"),notFound,"unlisted decks do not make a profile public");
    share(f.db,t,"public");
    assert.equal(functions.publicProfile(f.db,null,"second").display_name,"Second");
  } finally {f.db.close();}
});

test("a power-target decision's public summary names the power target (review S7)",()=>{
  assert.equal(state.publicSummary({kind:"accept",cards:[],document_edit_id:4},"power_target"),"Accepted a proposed change to the power target");
  assert.equal(state.publicSummary({kind:"reject",cards:[],document_edit_id:4},"thesis"),"Rejected a proposed change to the brief");
});

test("sharing controls: slugs are issued once, agents cannot publish, Archived decks can be shared",()=>{
  const f=fixture();
  try {
    const t=f.targets.find(t=>t.ui.user_id===1&&t.category==="paper")!;
    f.db.prepare("UPDATE decks SET slug=NULL WHERE id=?").run(t.id);
    assert.equal(functions.setVisibility(f.db,t.ui,t.id,"private").slug,null,"a private deck gets no slug");
    const slug=functions.setVisibility(f.db,t.ui,t.id,"unlisted").slug;
    assert.match(slug,/^[0-9A-Za-z]{10}$/);
    assert.equal(functions.setVisibility(f.db,t.ui,t.id,"public").slug,slug);
    assert.equal(functions.setVisibility(f.db,t.ui,t.id,"private").slug,slug,"going private keeps the link for later");
    assert.throws(()=>state.publicDeckState(f.db,f.users[1],slug),notFound);
    assert.throws(()=>functions.setVisibility(f.db,t.ag,t.id,"public"),PermissionError);
    assert.throws(()=>functions.setPublishDecisions(f.db,t.ag,t.id,true),PermissionError);
    assert.throws(()=>log.hideLogEntry(f.db,t.ag,t.log,true),PermissionError);
    assert.throws(()=>functions.setVisibility(f.db,t.ui,t.id,"everyone"),{status:400});
    decks.setDeckCategory(f.db,t.ui,t.id,"archived");
    assert.equal(functions.setVisibility(f.db,t.ui,t.id,"public").visibility,"public");
    assert.equal(functions.setPublishDecisions(f.db,t.ui,t.id,true).publish_decisions,true);
    assert.equal(log.hideLogEntry(f.db,t.ui,t.log,true).hidden,1);
    assert.equal(state.publicDeckState(f.db,null,slug).deck.category,"archived");
  } finally {f.db.close();}
});

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
      assert.throws(()=>decks.setDeckParent(db,ui,own[0].id,other[0].id),notFound);
      assert.throws(()=>decks.createDeck(db,ag,"Bad parent",{parentId:other[0].id}),notFound);
      assert.throws(()=>memory.remember(db,ag,{scope:"global",text:"Bad source",source_log_id:other[0].log}),notFound);
      assert.throws(()=>log.markReviewed(db,ag,{ids:[own[0].log,other[0].log]}),notFound);
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
    assert.equal(decks.listDecks(db,a.ag).find(d=>d.id===a.id)!.parent_id,null);
    assert.equal(isDeckOwner(db,a.ag,a.id),true);assert.equal(isDeckOwner(db,b.ag,a.id),false);
    assert.throws(()=>requireDeckOwner(db,b.ag,a.id),notFound);assert.throws(()=>requireSelf(a.ag,2),notFound);
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
  list_decisions:{kind:"read",args:t=>({deck:t.id})}, search_memories:{kind:"read",args:t=>({deck:t.id,query:"private"})},
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
  mark_decisions_reviewed:{kind:"write",args:t=>({through:t.log})},
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
        if(!c.global) assert.throws(()=>callTool({db:f.db,actor:t.ag},name,c.args(other,f)),notFound,`${name} other deck`);
        const run=()=>callTool({db:f.db,actor:t.ag},name,c.args(t,f));
        if(c.paperBlocked&&category==="paper") assert.throws(run,PermissionError);
        else { const result=run();assert.ok(result.text.length>0); }
      } finally {f.db.close();}
    }
  });
}

// Phase 4a viewer rows for the agent door: an id never reaches a shared deck,
// and a slug reaches one only through get_deck, compare_decks (other) and
// fork_deck. MCP has no anonymous caller: a missing bearer is 401 (below).
const SLUG_TOOLS = new Set(["get_deck","compare_decks","fork_deck"]);
for(const [name,c] of Object.entries(toolCases).filter(([,c])=>!c.global)) {
  test(`MCP ${name}: other user's agent on private, unlisted and public decks, by id and by slug`,()=>{
    const f=fixture();
    try {
      for(const t of f.targets.filter(t=>t.ui.user_id===1)) for(const [visibility,suggestions] of VISIBILITY_ROWS) {
        share(f.db,t,visibility,suggestions);
        const viewer=agent("test-agent",2), own=f.targets.find(x=>x.ui.user_id===2&&x.category===t.category)!;
        const before=personalSnapshot(f.db);
        assert.throws(()=>callTool({db:f.db,actor:viewer},name,c.args(t,f)),notFound,`${name} by id on ${visibility}`);
        assert.deepEqual(personalSnapshot(f.db),before);
        const bySlug=name==="compare_decks"?{deck:own.id,other:`/d/${slugOf(t)}`}:{...c.args(t,f),...("deck" in c.args(t,f)?{deck:slugOf(t)}:{})};
        if(!("deck" in c.args(t,f))) continue;
        const run=()=>callTool({db:f.db,actor:viewer},name,name==="fork_deck"?{...bySlug,name:`Viewer fork ${visibility}${suggestions} ${t.id}`}:bySlug);
        if(SLUG_TOOLS.has(name)&&visibility!=="private") {
          const text=run().text;
          for(const secret of PRIVATE_TEXT.filter(s=>s!=="test-agent")) assert.ok(!text.includes(secret),`${name} on a shared deck leaks "${secret}"`);
          if(name==="get_deck") assert.match(text,visibility==="public"?/\nPublic deck owned by owner; read-only/:/\nShared deck \(unlisted\) owned by owner; read-only/);
        } else {
          assert.throws(run,notFound,`${name} by slug on ${visibility}`);
          assert.deepEqual(personalSnapshot(f.db),before,`${name} by slug wrote nothing`);
        }
      }
    } finally {f.db.close();}
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
import { exportAccount } from "../src/accounts.ts";
import type { AddressInfo } from "node:net";
type HttpCase = { method:string; path:(t:Target,f:Fixture)=>string; kind:Classification; body?:(t:Target)=>unknown; global?:boolean; ownStatus?:number|((visibility:string)=>number);
  /** Addressed by slug: another signed-in user reaches it while the deck is unlisted or public (§18.3). */
  bySlug?:boolean;
  /** Without a session; public reads default to 200, everything else to 401 (§18.6). */
  anonymousStatus?:(visibility:string)=>number };
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
http("GET",()=>"/api/activity/latest","read",undefined,{global:true});
http("POST",()=>"/api/demo","owner-only",undefined,{global:true});
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
http("GET",t=>`/api/search?q=t:land&deck=${t.id}`,"read",undefined,{anonymousStatus:()=>404});
http("GET",()=>"/api/search?q=t:land","public-read",undefined,{global:true});
// Sharing (§18): the owner's controls, and the slug-addressed public surface.
http("PATCH",t=>`${deckPath(t)}/sharing`,"owner-only",()=>({visibility:"unlisted",publish_decisions:true}));
http("PATCH",t=>`${deckPath(t)}/log/${t.log}`,"owner-only",()=>({hidden:true}));
http("GET",(t,f)=>`${deckPath(t)}/compare?slug=${slugOf(f.otherOwn(t.ui,t))}`,"read");
const sharedRead=(v:string)=>v==="private"?404:200;
http("GET",t=>`/api/public/decks/${slugOf(t)}`,"public-read",undefined,{bySlug:true,anonymousStatus:sharedRead});
http("GET",t=>`/api/public/decks/${slugOf(t)}/export`,"public-read",undefined,{bySlug:true,anonymousStatus:sharedRead});
// A profile exists only while its person has a public deck (review S3).
http("GET",()=>"/api/public/users/owner","public-read",undefined,{global:true,ownStatus:v=>v==="public"?200:404,anonymousStatus:v=>v==="public"?200:404});
http("POST",t=>`/api/public/decks/${slugOf(t)}/fork`,"write",()=>({name:`HTTP shared fork ${randomUUID()}`}),{bySlug:true});
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
    for(const category of ["paper","theorycrafted"]) for(const visibility of ["private","unlisted","public"]) for(const c of httpCases) {
      const own=f.targets.find(t=>t.ui.user_id===1&&t.category===category)!;
      // Each mutation runs in a savepoint and rolls back, so every row sees
      // the same meaningful setup without hiding the request's validation.
      f.db.exec("SAVEPOINT http_matrix");
      try {
        principal=OWNER;
        const other=f.targets.find(t=>t.ui.user_id===2&&t.category===category)!;
        // Both decks share the visibility under test; ids stay the owner's namespace.
        f.db.prepare("UPDATE decks SET visibility=? WHERE id IN (?,?)").run(visibility,own.id,other.id);
        const label=`${c.method} ${c.path(own,f)} (${visibility} ${category})`;
        if(!c.global) {
          const res=await send(base,c,other,f);
          assert.equal(res.status,c.bySlug&&visibility!=="private"?200:404,`${label} cross-user: ${await res.text()}`);
        }
        const res=await send(base,c,own,f);
        assert.equal(res.status,typeof c.ownStatus==="function"?c.ownStatus(visibility):c.ownStatus??200,`${label} own: ${await res.text()}`);
        principal=null;
        const anonymous=await send(base,c,own,f);
        assert.equal(anonymous.status,c.anonymousStatus?.(visibility)??(c.kind==="public-read"?200:401),`${label} anonymous: ${await anonymous.text()}`);
      } finally { f.db.exec("ROLLBACK TO http_matrix; RELEASE http_matrix"); }
    }
  } finally {await app.close();f.db.close();}
});

test("hosted UI APIs await sessions; health stays public; settings omit local stdio commands",async()=>{
  const f=fixture(),saved=process.env.DECKBUILDER_PUBLIC_ORIGIN;
  let principal:Actor|null=null;
  const app=createApp(f.db,{mode:"hosted",resolveUiActor:()=>principal}),base=await listening(app);
  process.env.DECKBUILDER_PUBLIC_ORIGIN=base;
  try {
    const token=tokens.createToken(f.db,OWNER,"Hosted agent").token;
    assert.equal((await fetch(base+"/api/health")).status,200);
    for(const path of ["/api/decks","/api/settings","/api/events"]) assert.equal((await fetch(base+path,{headers:{Authorization:`Bearer ${token}`}})).status,401);
    assert.equal((await fetch(base+"/mcp")).status,401);
    principal=f.users[1]; // exercise the Phase 2 session seam in hosted mode
    const settings=await fetch(base+"/api/settings");assert.equal(settings.status,200);
    assert.equal("stdio_command" in await settings.json(),false);
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
    assert.equal(forbidden.isError,true);assert.match(JSON.stringify(forbidden.content),/There is no deck/);
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

/** Snapshot all personal tables (including FTS shadows and sequences), not
 * just the requested deck. A rejected call must have no partial side effects. */
function personalSnapshot(db: Database) {
  const tables=db.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").all() as {name:string}[];
  return Object.fromEntries(tables.filter(({name})=>!name.startsWith("cards")&&!['card_names','card_prices','meta'].includes(name))
    .map(({name})=>[name,db.prepare(`SELECT * FROM "${name.replaceAll('"','""')}"`).all().sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)))]));
}
type MixedProbe={name:string;run:(f:Fixture,a:Actor,own:Target,foreign:Target)=>unknown;human?:boolean};
const mixedProbes:MixedProbe[]=[
  {name:"updateSlot slotId",run:(f,a,o,x)=>decks.updateSlot(f.db,a,o.id,x.slot,{name:"Wrong slot"})},
  {name:"deleteSlot slotId",run:(f,a,o,x)=>decks.deleteSlot(f.db,a,o.id,x.slot)},
  {name:"renameTag tagId",run:(f,a,o,x)=>decks.renameTag(f.db,a,o.id,x.tag,"Wrong tag")},
  {name:"deleteTag tagId",run:(f,a,o,x)=>decks.deleteTag(f.db,a,o.id,x.tag)},
  {name:"organizeCard primary slot",run:(f,a,o,x)=>decks.organizeCard(f.db,a,o.id,"id-forest",{slotId:x.slot})},
  {name:"organizeCard secondary slot",run:(f,a,o,x)=>decks.organizeCard(f.db,a,o.id,"id-forest",{secondarySlotIds:[x.slot]})},
  {name:"organizeCard tags",run:(f,a,o,x)=>decks.organizeCard(f.db,a,o.id,"id-forest",{tagIds:[o.tag,x.tag]})},
  {name:"validateSlotAssignments primary",run:(f,a,o,x)=>decks.validateSlotAssignments(f.db,a,o.id,x.slot,[])},
  {name:"validateSlotAssignments secondary",run:(f,a,o,x)=>decks.validateSlotAssignments(f.db,a,o.id,null,[x.slot])},
  {name:"removeEngine engineId",human:true,run:(f,a,o,x)=>brief.removeEngine(f.db,a,o.id,x.engine)},
  ...["primary","secondary"].flatMap(kind=>[
    {name:`applyChanges ${kind} slot`,run:(f:Fixture,a:Actor,o:Target,x:Target)=>membership.applyChanges(f.db,a,o.id,[{action:"add",oracle_id:"id-solring",...(kind==="primary"?{slot_id:x.slot}:{secondary_slot_ids:[x.slot]})}],{kind:"edit"})},
    {name:`createProposal ${kind} slot`,run:(f:Fixture,a:Actor,o:Target,x:Target)=>proposals.createProposal(f.db,a,o.id,{items:[{action:"cut",oracle_id:"id-forest",reason:"Valid first item"},{action:"add",oracle_id:"id-solring",reason:"Foreign slot",...(kind==="primary"?{slot_id:x.slot}:{secondary_slot_ids:[x.slot]})}]})},
  ]),
  {name:"memory id read",run:(f,a,_o,x)=>memory.getMemory(f.db,a,x.memory)},
  ...["update","retire","restore","confirm"].map(action=>({name:`reviseMemory ${action} foreign id`,run:(f:Fixture,a:Actor,_o:Target,x:Target)=>memory.reviseMemory(f.db,a,x.memory,{action:action as "retire"})})),
  {name:"reviseMemory replacement deck",run:(f,a,o,x)=>memory.reviseMemory(f.db,a,o.memory,{action:"supersede",scope:"deck",deck_id:x.id,text:"Wrong replacement scope"})},
  {name:"remember source log",run:(f,a,o,x)=>memory.remember(f.db,a,{scope:"deck",deck_id:o.id,source_log_id:x.log,text:"Wrong source"})},
  {name:"undo logId",human:true,run:(f,a,_o,x)=>membership.undo(f.db,a,x.log)},
  {name:"getLogEntry logId",run:(f,a,_o,x)=>log.getLogEntry(f.db,a,x.log)},
  {name:"markReviewed mixed IDs",run:(f,a,o,x)=>log.markReviewed(f.db,a,{ids:[o.log,x.log]})},
  {name:"markReviewed through",run:(f,a,_o,x)=>log.markReviewed(f.db,a,{through:x.log})},
  {name:"writeLog proposal link",run:(f,a,o,x)=>log.writeLog(f.db,a,{deck_id:o.id,kind:"edit",proposal_id:x.proposal})},
  {name:"writeLog edit link",run:(f,a,o,x)=>log.writeLog(f.db,a,{deck_id:o.id,kind:"edit",document_edit_id:x.edit})},
  {name:"writeLog undo link",run:(f,a,o,x)=>log.writeLog(f.db,a,{deck_id:o.id,kind:"edit",undo_of:x.log})},
  {name:"acceptItems mixed IDs",human:true,run:(f,a,o,x)=>proposals.acceptItems(f.db,a,[o.item,x.item])},
  {name:"rejectItems mixed IDs",human:true,run:(f,a,o,x)=>proposals.rejectItems(f.db,a,[o.item,x.item])},
  {name:"acceptProposals foreign proposal",human:true,run:(f,a,o,x)=>proposals.acceptProposals(f.db,a,o.id,[{proposal_id:o.proposal},{proposal_id:x.proposal}],1)},
  {name:"getProposal forWrite",run:(f,a,_o,x)=>proposals.getProposal(f.db,a,x.proposal,true)},
  {name:"withdrawProposal foreign id",run:(f,a,_o,x)=>proposals.withdrawProposal(f.db,a,x.proposal,"Wrong owner")},
  {name:"amendProposal foreign id",run:(f,a,_o,x)=>proposals.amendProposal(f.db,a,x.proposal,{items:[{action:"cut",oracle_id:"id-forest",reason:"Wrong owner"}]})},
  {name:"getDocumentEdit forWrite",run:(f,a,_o,x)=>documents.getDocumentEdit(f.db,a,x.edit,true)},
  {name:"withdrawDocumentEdit foreign id",run:(f,a,_o,x)=>documents.withdrawDocumentEdit(f.db,a,x.edit,"Wrong owner")},
  {name:"ruleDocumentEdit foreign id",human:true,run:(f,a,_o,x)=>documents.ruleDocumentEdit(f.db,a,x.edit,"reject")},
  {name:"proposeDocumentGroup foreign supersedes",run:(f,a,o,x)=>documents.proposeDocumentGroup(f.db,a,o.id,[{kind:"constraints",payload:{content:"Wrong supersession"},reason:"Wrong owner"}],{supersedes:[x.edit]})},
  {name:"compareDecks other",run:(f,a,o,x)=>fork.compareDecks(f.db,a,o.id,x.id)},
  {name:"compareDeckSources sources",run:(f,a,o,x)=>sourcing.compareDeckSources(f.db,a,o.id,[x.id])},
  {name:"compareDeckSources source tags",run:(f,a,o,x)=>sourcing.compareDeckSources(f.db,a,o.id,[x.id],{apply_source_tags:true,expected_revision:1,expected_source_revisions:{[x.id]:1}})},
  {name:"copyPowerTarget destination",run:(f,a,o,x)=>power.copyPowerTarget(f.db,a,o.id,x.id)},
  {name:"copyDeckMemories destination",run:(f,a,o,x)=>memory.copyDeckMemories(f.db,a,o.id,x.id)},
  {name:"revokeToken foreign credential",human:true,run:(f,a,_o,x)=>tokens.revokeToken(f.db,a,x.token)},
];
for(const probe of mixedProbes) test(`mixed IDs: ${probe.name}, own deck with foreign child; every tenant table unchanged`,()=>{
  const f=fixture();
  try {
    for(const userId of [1,2]) for(const via of ["ui","agent"] as const) {
      const own=f.targets.find(t=>t.ui.user_id===userId&&t.category==="theorycrafted")!,foreign=f.targets.find(t=>t.ui.user_id!==userId&&t.category==="theorycrafted")!;
      const a=via==="ui"?own.ui:own.ag,before=personalSnapshot(f.db);
      assert.throws(()=>probe.run(f,a,own,foreign),via==="agent"&&probe.human?(e:any)=>e.status===403||e.status===404:notFound);
      assert.deepEqual(personalSnapshot(f.db),before,`${probe.name}: no tenant, FTS or sequence changes`);
    }
  } finally {f.db.close();}
});

test("MCP and HTTP reject own-deck/foreign-child combinations without partial writes",async()=>{
  const f=fixture(),own=f.targets[1],foreign=f.targets[3];
  const app=createApp(f.db),base=await listening(app);
  const requests=[
    {method:"PATCH",path:`/api/decks/${own.id}/slots/${foreign.slot}`,body:{name:"Cross slot"}},
    {method:"DELETE",path:`/api/decks/${own.id}/slots/${foreign.slot}`},
    {method:"PATCH",path:`/api/decks/${own.id}/tags/${foreign.tag}`,body:{name:"Cross tag"}},
    {method:"DELETE",path:`/api/decks/${own.id}/tags/${foreign.tag}`},
    ...[{slot_id:foreign.slot},{secondary_slot_ids:[foreign.slot]},{tag_ids:[own.tag,foreign.tag]}].map(body=>({method:"PATCH",path:`/api/decks/${own.id}/cards/id-forest`,body})),
    {method:"DELETE",path:`/api/decks/${own.id}/engines/${foreign.engine}`},
    {method:"POST",path:`/api/decks/${own.id}/cards`,body:{oracle_id:"id-solring",slot_id:foreign.slot}},
    {method:"POST",path:`/api/decks/${own.id}/items/rule`,body:{item_ids:[own.item,foreign.item],outcome:"accept"}},
    {method:"POST",path:`/api/decks/${own.id}/proposals/${foreign.proposal}/rule`,body:{outcome:"reject"}},
    {method:"POST",path:`/api/decks/${own.id}/log/${foreign.log}/undo`},
    {method:"GET",path:`/api/decks/${own.id}/compare?other=${foreign.id}`},
  ];
  try {
    for(const req of requests){
      const before=personalSnapshot(f.db),res=await fetch(base+req.path,{method:req.method,headers:req.body?{"Content-Type":"application/json"}:{},...(req.body?{body:JSON.stringify(req.body)}:{})});
      assert.equal(res.status,404,`${req.method} ${req.path}: ${await res.text()}`);
      assert.deepEqual(personalSnapshot(f.db),before);
    }
    for(const [name,args] of [["compare_decks",{deck:own.id,other:foreign.id}],["compare_deck_sources",{deck:own.id,sources:[foreign.id]}],["propose_document_changes",{deck:own.id,changes:[{kind:"constraints",text:"Wrong replacement",reason:"Wrong owner"}],supersedes:[foreign.edit]}]] as const){
      const before=personalSnapshot(f.db);assert.throws(()=>callTool({db:f.db,actor:own.ag},name,args),notFound);assert.deepEqual(personalSnapshot(f.db),before);
    }
  } finally {await app.close();f.db.close();}
});

// Every non-deck export has a concrete user-2 probe. The invocation proxy
// rejects a no-op probe, and output/owner snapshots verify the privacy claim.
const globalProbes:Record<string,GlobalProbe>={
  accountInfo:(fn,f,a)=>{const info=fn(f.db,a);assert.equal(info.id,2);return info;},
  chooseHandle:(fn,f,a)=>{f.db.prepare("UPDATE users SET needs_handle=1 WHERE id=?").run(a.user_id);return fn(f.db,a,"second_chosen");},
  connectedClients:(fn,f,a)=>fn(f.db,a),
  disconnectClient:(fn,f,a)=>{assert.throws(()=>fn(f.db,a,"missing"),{status:404});return null;},
  ipBucket:fn=>fn("2001:db8::1"),
  exportAccount:(fn,f,a)=>{const data=fn(f.db,a);assert.ok(data.tables.decks.every((d:any)=>d.user_id===2));return data;},
  deleteAccount:(fn,f,a)=>{const handle=f.db.prepare("SELECT handle FROM users WHERE id=2").get()!.handle;fn(f.db,a,handle);assert.equal(f.db.prepare("SELECT id FROM users WHERE id=2").get(),undefined);return null;},
  createInvite:(fn,f,a)=>{assert.throws(()=>fn(f.db,a),{status:403});return null;},
  adminInfo:(fn,f,a)=>{assert.throws(()=>fn(f.db,a),{status:403});return null;},
  accountScopes:(fn,f)=>{const rows=fn(f.db);assert.ok(rows.some((r:any)=>r.table==="decks"));return rows;},
  clientIp:fn=>fn({headers:{"x-forwarded-for":"attacker"},socket:{remoteAddress:"127.0.0.1"}},{}),
  securityHeaders:fn=>{const headers:Record<string,string>={};fn({setHeader:(k:string,v:string)=>{headers[k]=v;}},"local");assert.deepEqual(headers,{});return headers;},
  consentHtml:fn=>{const html=fn({name:"<script>bad</script>",identityHost:null,redirectHost:"127.0.0.1",redirectUri:"http://127.0.0.1/cb",loopback:true});assert.ok(html.includes("&lt;script&gt;"));return html;},
  uiPreferences:(fn,f,a)=>{ const result=fn(f.db,a); assert.equal(result.has_agent_activity,true); return result; },
  setAdvancedOpen:(fn,f,a)=>{ assert.throws(()=>fn(f.db,{...a,via:"agent"},true),PermissionError); fn(f.db,a,true); return getSetting(f.db,a,"advanced_open"); },
  createDemo:(fn,f,a)=>{ assert.throws(()=>fn(f.db,{...a,via:"agent"}),PermissionError); const result=fn(f.db,a); return decks.requireDeck(f.db,a,result.deckId); },
  getSetting:(fn,f,a)=>{setSetting(f.db,a,"probe","second setting");return fn(f.db,a,"probe");},
  setSetting:(fn,f,a)=>{fn(f.db,a,"probe","second setting");assert.equal(getSetting(f.db,a,"probe"),"second setting");return getSetting(f.db,a,"probe");},
  createDeck:(fn,f,a)=>{const id=fn(f.db,a,"Second new list");return decks.requireDeck(f.db,a,id);},
  listDecks:(fn,f,a)=>{const rows=fn(f.db,a);assert.deepEqual(rows.map((r:any)=>r.id),f.targets.filter(t=>t.ui.user_id===2).map(t=>t.id));return rows;},
  getProfile:(fn,f,a)=>{const p=fn(f.db,a);assert.equal(p.body_md,"Private profile 2");return p;},
  setProfile:(fn,f,a)=>{fn(f.db,a,"Second changed profile");return documents.getProfile(f.db,a);},
  getBudgets:(fn,f,a)=>{const b=fn(f.db,a);assert.equal(b.global,1002);return b;},
  setBudgets:(fn,f,a)=>{fn(f.db,a,{global:1900});return memory.getBudgets(f.db,a);},
  unreviewedCount:(fn,f,a)=>{log.markReviewed(f.db,OWNER,{through:f.targets[0].log});assert.equal(fn(f.db,a),1);return fn(f.db,a);},
  getFocus:(fn,f,a)=>{const focus=fn(f.db,a);assert.equal(focus.deck_id,f.targets[2].id);return focus;},
  playerStats:(fn,f,a)=>{
    const paper=f.targets[2];
    // Different counts, slot names, commander colors/types and reuse make
    // accidental aggregation detectable beyond the coarse deck_count.
    membership.applyChanges(f.db,a,paper.id,[{action:"add",oracle_id:"id-teferi",role:"commander"},{action:"add",oracle_id:"id-counterspell"}],{kind:"edit"});
    decks.createSlot(f.db,a,paper.id,"Second distinctive slot");
    const result=fn(f.db,a),v=decks.getDeck(f.db,a,paper.id);
    assert.deepEqual(result.decks.map((d:any)=>d.id),[paper.id]);
    assert.equal(result.decks[0].card_count,v.computed.card_count);
    assert.deepEqual(result.decks[0].commanders,v.cards.filter(c=>c.role==="commander").map(c=>c.name));
    assert.equal(result.families,1);assert.equal(result.averages.lands,7);
    assert.deepEqual(result.colors,{W:0,U:1,B:0,R:0,G:0});
    assert.deepEqual(result.identities,[{identity:"U",decks:1}]);
    assert.ok(result.slots.some((s:any)=>s.name==="Second distinctive slot"));
    assert.deepEqual(result.reused_cards,[]);
    return result;
  },
  listActivity:(fn,f,a)=>{const rows=fn(f.db,a);assert.ok(rows.length);assert.ok(rows.every((r:any)=>r.user_id===2));return rows;},
  latestActivity:(fn,f,a)=>{const row=fn(f.db,a);assert.equal(row.user_id,2);return row;},
  activityPage:(fn,f,a)=>{const page=fn(f.db,a);assert.ok(page.entries.length);assert.ok(page.entries.every((r:any)=>r.user_id===2));assert.deepEqual(page.actors,["test-agent"]);return page;},
  ownershipEvidence:(fn,f,a)=>{const rows=fn(f.db,a,"id-forest");assert.ok(rows.length);assert.ok(rows.every((r:any)=>r.user_id===2));return rows;},
  createToken:(fn,f,a)=>{const row=fn(f.db,a,"second credential");assert.equal(row.credential.user_id,2);return row;},
  listTokens:(fn,f,a)=>{const rows=fn(f.db,a);assert.ok(rows.length);assert.ok(rows.every((r:any)=>r.user_id===2));return rows;},
  revokeToken:(fn,f,a,t)=>{fn(f.db,a,t.token);const row=tokens.listTokens(f.db,a).find(r=>r.id===t.token)!;assert.ok(row.revoked_at);return row;},
  verifyToken:(fn,f,a)=>{const token=tokens.createToken(f.db,a,"verify second");const actor=fn(f.db,token.token);assert.equal(actor.user_id,2);return actor;},
  migrateLegacyToken:(fn,f)=>{
    // Operator-only import explicitly targets user 1; probe an isolated fresh
    // database instead of relabelling it as a tenant-capability authorization.
    const isolated=functions.openDb(":memory:"),dir=mkdtempSync(join(tmpdir(),"import-probe-"));
    try {
      const file=join(dir,"agent-token");writeFileSync(file,"isolated-existing-credential",{mode:0o600});
      const result=fn(isolated,file);assert.equal(result,undefined);
      assert.equal(isolated.prepare("SELECT count(*) n FROM agent_tokens").get().n,1);
      assert.equal(readFileSync(join(dir,"stdio-token"),"utf8"),"isolated-existing-credential");return result;
    }finally{isolated.close();rmSync(dir,{recursive:true,force:true});}
  },
  tableCard:(fn,f,a)=>fn(f.db,a,"id-forest"),
  cardsNamed:(fn,f)=>fn(f.db,"Forest"),requireCardById:(fn,f)=>fn(f.db,"id-forest"),
  resolveCard:(fn,f)=>fn(f.db,"Forest"),resolveCards:(fn,f)=>fn(f.db,["Forest"]),tryResolveCard:(fn,f)=>fn(f.db,"Forest"),
  suggestNames:(fn,f)=>fn(f.db,"Fores"),extractCardRefs:fn=>fn("Uses [[Forest]]"),lintProse:(fn,f)=>fn(f.db,"Uses [[Forest]]"),
  copyLimit:(fn,f)=>fn(functions.requireCardById(f.db,"id-forest")),similarTag:fn=>fn(["Have"],"have"),validateVocabName:fn=>fn("Utility"),
  isDoctorPair:fn=>fn([]),parseDeckList:fn=>fn("7 Forest"),
  parseTarget:fn=>fn({profile:{}}),mapBracket:fn=>fn(power.parseTarget({profile:{}}).profile,0),
  profileLines:fn=>fn(power.parseTarget({profile:{}}).profile),targetText:fn=>fn(null),estimateTokens:fn=>fn("Second preference"),
  authorOf:(fn,_f,a)=>fn(a),displayAuthor:(fn,f)=>{const s=fn(f.db,{author_user_id:2,author_via:"agent",author_label:"second agent"});assert.equal(s,"second via second agent");return s;},
  isAuthor:(fn,_f,a)=>{assert.equal(fn(a,{author_user_id:1,author_via:"ui",author_label:"ui"}),false);return fn(a,{author_user_id:2,author_via:"ui",author_label:"ui"});},
  requireSelf:(fn,_f,a)=>{assert.throws(()=>fn(a,1),notFound);return fn(a,2);},
  requireUi:(fn,_f,a)=>fn(a,"perform a human action"),
  requireActor:(fn,_f,a)=>{assert.throws(()=>fn(null),{status:401});return fn(a);},requireAdmin:(fn,f,a)=>{assert.throws(()=>fn(f.db,a),PermissionError);return false;},
  normalizeName:fn=>fn("Second"),isUniqueViolation:fn=>fn(Object.assign(new Error("unique"),{errcode:2067})),
  getMode:fn=>fn({DECKBUILDER_MODE:"local"}),publicOrigin:fn=>fn({DECKBUILDER_PUBLIC_ORIGIN:"https://second.example"}),
  localActor:fn=>{const actor=fn();assert.deepEqual(actor,{user_id:1,via:"ui",label:"ui"});return undefined;}, // explicit local-mode policy, no stored rows
  allowedRequest:fn=>fn({headers:{host:"127.0.0.1:8787"}},"local"),
  openDb:fn=>{const db=fn(":memory:");try{assert.equal(db.prepare("SELECT count(*) n FROM decks").get().n,0);return [];}finally{db.close();}},
  withTransaction:(fn,f,a)=>fn(f.db,()=>documents.getProfile(f.db,a)),
  getPriceSource:(fn,f,a)=>{assert.deepEqual(fn(f.db,null),{currency:"USD",store:"lowest"});setSetting(f.db,a,"price_currency","EUR");assert.deepEqual(fn(f.db,a),{currency:"EUR",store:"lowest"});return fn(f.db,a);},
  setPriceSource:(fn,f,a)=>{assert.throws(()=>fn(f.db,null,{currency:"USD",store:"lowest"}),{status:401});assert.throws(()=>fn(f.db,{...a,via:"agent",label:"probe-agent"},{currency:"EUR",store:"cardmarket"}),PermissionError);fn(f.db,a,{currency:"EUR",store:"cardmarket"});assert.equal(getSetting(f.db,a,"price_currency"),"EUR");return getSetting(f.db,a,"price_store");},
  priceResolver:(fn,f,a)=>{const anon=fn(f.db,null);assert.equal(anon.currency,"USD");assert.equal(anon.store,"tcgplayer");setSetting(f.db,a,"price_currency","EUR");const r=fn(f.db,a);assert.equal(r.store,"cardmarket");return r.resolve([{oracle_id:"id-forest"}]);},
  priceSourceLine:fn=>fn({currency:"EUR",store:"cardmarket",as_of:null}),
  pricesAsOf:(fn,f)=>fn(f.db),priceStatus:(fn,f)=>fn(f.db),
  formatPrice:fn=>fn(100),estimateCost:fn=>fn([{quantity:2,cents:100}]),cheapestPrintings:fn=>fn([]),
  ingestPrices:(fn,f)=>fn(f.db,new Map(),"2026-10-25T00:00:00Z"),
  parse:fn=>fn("t:land"),compile:fn=>fn(functions.parse("t:land")),
  cardLine:(fn,f)=>fn(functions.requireCardById(f.db,"id-forest")),cardWithContext:(fn,f)=>fn(functions.requireCardById(f.db,"id-forest")),
  memoryLine:(fn,f,a,t)=>fn(memory.getMemory(f.db,a,t.memory)),renderMemories:(fn,f,a)=>fn(memory.listMemories(f.db,a)),
  decisionLine:(fn,f,a,t)=>fn(log.cardHistory(f.db,a,t.id,"id-forest")[0]),
  findingLines:(fn,f,a,t)=>fn(checks.checkDeck(f.db,a,t.id)),
  logLines:(fn,f,a)=>fn(log.listLog(f.db,a)),proposalLines:(fn,f,a)=>fn(proposals.listProposals(f.db,a),documents.listDocumentEdits(f.db,a)),
  renderActivity:(fn,f,a)=>fn(activity.listActivity(f.db,a)),renderStats:(fn,f,a)=>fn(stats.playerStats(f.db,a)),
  rosterLines:(fn,f,a)=>fn(decks.listDecks(f.db,a)),renderScope:(fn,f,a)=>fn(memory.loadScope(f.db,a,"global"),"global"),
  newSlug:fn=>{const slug=fn();assert.match(slug,/^[0-9A-Za-z]{10}$/);return slug;},
  slugFrom:fn=>{
    for(const ok of ["https://example.test/d/Ab3dE6gH9k","HTTPS://example.test/d/Ab3dE6gH9k/","<https://example.test/d/Ab3dE6gH9k>","https://example.test/d/Ab3dE6gH9k.","/d/Ab3dE6gH9k","Ab3dE6gH9k"]) assert.equal(fn(ok),"Ab3dE6gH9k",ok);
    for(const no of ["Second deck","https://example.test/x/Ab3dE6gH9k","https://example.test/Ab3dE6gH9k","d/Ab3dE6gH9k/extra"]) assert.equal(fn(no),null,no);
    return fn("/d/Ab3dE6gH9k");},
  publicProfile:(fn,f,a,t)=>{f.db.prepare("UPDATE decks SET visibility='public' WHERE id=?").run(t.id);const p=fn(f.db,a,"second");assert.deepEqual(p.decks.map((d:any)=>d.slug),[slugOf(t)]);return p;},
  sharedCounts:(fn,f,a,t)=>{f.db.prepare("UPDATE decks SET visibility='unlisted' WHERE id=?").run(t.id);const c=fn(f.db,a);assert.deepEqual(c,{public:0,unlisted:1});return c;},
  linkPreview:(fn,f,a,t)=>{const p=fn(f.db,a,`/d/${slugOf(t)}`,"https://example.test");assert.equal(p.status,200);assert.equal(fn(f.db,a,`/d/${slugOf(f.targets[0])}`,"https://example.test").status,404);return p;},
  withPreview:fn=>fn("<head><title>x</title></head>",{status:200,head:"<title>Second</title>"}),
  renderPublicDeck:(fn,f,a,t)=>fn(state.publicDeckState(f.db,a,slugOf(t))),
  publicParent:(fn,f,a,t)=>{f.db.prepare("UPDATE decks SET visibility='public' WHERE id=?").run(t.id);const p=fn(f.db,{owner_handle:"second",slug:slugOf(t),name:"x"});assert.equal(p.slug,slugOf(t));return p;},
  decisionReason:fn=>{assert.equal(fn("  a reason  "),"a reason");assert.equal(fn("   "),"");assert.throws(()=>fn("x".repeat(501)),{status:400});return fn("Second");},
  publicSummary:fn=>{const s=fn({kind:"accept",document_edit_id:3,cards:[]});assert.equal(s,"Accepted a proposed change to the brief");return s;},
  renderPower:(fn,f,a,t)=>fn(power.getPower(f.db,a,t.id)),stateLines:(fn,f,a,t)=>fn(decks.getDeck(f.db,a,t.id)),
  renderCompare:(fn,f,a,t)=>fn(fork.compareDecks(f.db,a,t.id,f.otherOwn(a,t).id)),
  resolutionFailureLine:fn=>fn({ref:"missing",problem:"unknown",suggestions:[],candidates:[]}),
  shuffle:fn=>fn(["second"]),defaultRow:(fn,f,a)=>fn(playtest.tableCard(f.db,a,"id-forest")),
  makeObject:(fn,f,a)=>fn(playtest.tableCard(f.db,a,"id-forest")),bottomRequired:(fn,_f,_a,t)=>fn(t.session.session.frame),
  view:(fn,_f,_a,t)=>fn(t.session.session),zoneOf:(fn,_f,_a,t)=>fn(t.session.session.frame,t.session.session.frame.zones.hand[0]),
  applyAction:(fn,_f,_a,t)=>{const session=structuredClone(t.session.session);fn(session,{type:"keep"});return session;},
};
for(const name of ["AppError","ServiceError","SchemaError","PermissionError","SearchError"])globalProbes[name]=fn=>new fn("Second probe");
globalProbes.CardResolutionError=fn=>new fn([{ref:"missing",problem:"unknown",suggestions:[],candidates:[]}]);
const globalKinds:Record<string,Classification>={createDemo:"owner-only",uiPreferences:"read",setAdvancedOpen:"owner-only",sharedCounts:"read",getPriceSource:"read",setPriceSource:"owner-only",priceResolver:"read",connectedClients:"owner-only",disconnectClient:"owner-only",accountInfo:"owner-only",chooseHandle:"owner-only",exportAccount:"owner-only",deleteAccount:"owner-only",createInvite:"owner-only",adminInfo:"owner-only",accountScopes:"public-read",getSetting:"read",setSetting:"write",createDeck:"write",listDecks:"read",getProfile:"read",setProfile:"owner-only",getBudgets:"read",setBudgets:"write",unreviewedCount:"read",getFocus:"read",playerStats:"read",listActivity:"read",latestActivity:"read",activityPage:"read",ownershipEvidence:"read",createToken:"owner-only",listTokens:"owner-only",revokeToken:"owner-only",requireUi:"owner-only",requireAdmin:"owner-only",ingestPrices:"write"};
for(const [name,probe] of Object.entries(globalProbes))globalCase(name,globalKinds[name]??"public-read",probe);

function ownerSnapshot(f:Fixture) {
  const owned=new Set(f.targets.filter(t=>t.ui.user_id===1).map(t=>t.id));
  const memoryIds=new Set((f.db.prepare("SELECT id FROM memories WHERE user_id=1").all() as {id:number}[]).map(r=>r.id));
  const proposalsIds=new Set((f.db.prepare("SELECT p.id FROM proposals p JOIN decks d ON d.id=p.deck_id WHERE d.user_id=1").all() as {id:number}[]).map(r=>r.id));
  const engines=new Set(f.targets.filter(t=>t.ui.user_id===1).map(t=>t.engine));
  return Object.fromEntries(Object.entries(personalSnapshot(f.db)).filter(([name])=>!name.startsWith("memories_fts_")&&name!=="sqlite_sequence").map(([name,rows])=>[name,rows.filter((r:any)=>{
    if(name==="users")return r.id===1;
    if(r.user_id!=null)return r.user_id===1;
    if(r.deck_id!=null)return owned.has(r.deck_id);
    if(r.memory_id!=null)return memoryIds.has(Number(r.memory_id));
    if(r.proposal_id!=null)return proposalsIds.has(r.proposal_id);
    if(r.engine_id!=null)return engines.has(r.engine_id);
    return false;
  })]));
}
function assertNoOwnerRows(value:unknown,f:Fixture) {
  if(value instanceof Map){for(const v of value.values())assertNoOwnerRows(v,f);return;}
  if(Array.isArray(value)){for(const v of value)assertNoOwnerRows(v,f);return;}
  if(value&&typeof value==="object") {
    const row=value as Record<string,unknown>;
    for(const key of ["user_id","author_user_id","actor_user_id"])if(row[key]!=null)assert.notEqual(row[key],1,key);
    for(const v of Object.values(row))assertNoOwnerRows(v,f);
  }
  if(typeof value==="string")for(const privateText of ["Private profile 1","Private activity 1","Private global preference 1","Private deck preference 1","Private deck preference 2"])assert.ok(!value.includes(privateText),privateText);
}
test("glob discovery classifies every runtime export, including renderers/cards/prices and future modules",()=>{
  assert.deepEqual(Object.keys(cases).sort(),Object.keys(functions).sort());
  for(const [name,c]of Object.entries(cases))if(c.global)assert.equal(typeof c.probe,"function",`${name} requires a real user-2 probe`);
});
for(const [name,c]of Object.entries(cases).filter(([,c])=>c.global))test(`global probe ${name}: runs export as user 2, returns no user-1 rows`,async()=>{
  const f=fixture(),a=f.users[1],t=f.targets[3];
  try {
    // Review queues differ to make count leaks visible without modifying user 1 during the probe.
    log.markReviewed(f.db,OWNER,{through:f.targets[0].log});
    const before=ownerSnapshot(f),result=await c.run(f.db,a,t,f);
    assertNoOwnerRows(result,f);assert.deepEqual(ownerSnapshot(f),before,`${name} changed user 1`);
  } finally {f.db.close();}
});
