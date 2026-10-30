// Phase 1 of the §4 matrix: all decks are private. Visibility/suggestions
// extend this dimension in Phase 4a; anonymous access is tested at both doors.
import { globSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { randomUUID } from "node:crypto";
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Database } from "../src/sqlite.ts";
import { type Actor, PermissionError, isDeckOwner, requireDeckOwner, requireSelf } from "../src/actor.ts";
import * as decks from "../src/deck/service.ts";
import * as brief from "../src/deck/brief.ts";
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
import { PKG_ROOT, getSetting, setSetting } from "../src/db.ts";
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

for(const [name,c] of Object.entries(cases).filter(([name])=>["getDeck","applyChanges","updateBrief","setDeckCategory","undo","deleteDeck"].includes(name))) {
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
function share(db: Database, t: Target, visibility: string, suggestions = 0, category: string = t.category) {
  db.prepare("UPDATE decks SET visibility=?, accept_suggestions=?, category=? WHERE id=?").run(visibility, suggestions, category, t.id);
}
const viewers = (t: Target): Array<[string, Actor | null]> => [
  ["other-ui", { user_id: 2, via: "ui", label: "ui" }], ["other-agent", agent("test-agent", 2)], ["anonymous", null],
].filter(([, a]) => a === null || (a as Actor).user_id !== t.ui.user_id) as Array<[string, Actor | null]>;
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


function personalSnapshot(db: Database) {
  const tables=db.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").all() as {name:string}[];
  return Object.fromEntries(tables.filter(({name})=>!name.startsWith("cards")&&!['card_names','card_prices','meta'].includes(name))
    .map(({name})=>[name,db.prepare(`SELECT * FROM "${name.replaceAll('"','""')}"`).all().sort((a,b)=>JSON.stringify(a).localeCompare(JSON.stringify(b)))]));
}
