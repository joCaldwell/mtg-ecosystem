// packages/oracle-parser/tests/parser.test.ts
import { describe, it, expect } from "vitest";
import { parseOracleText } from "../src/index.js";
import { normalizeOracleText } from "../src/ingest/normalize.js";

describe("Oracle Text Parser", () => {
  describe("Baseline Tests", () => {
    it("should parse keyword abilities", () => {
      const tree = parseOracleText("Flying");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Flying<EOF>");
    });

    it("should parse activated abilities", () => {
      const tree = parseOracleText("{T}: Draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("{T}:Drawacard.<EOF>");
    });

    it("should parse triggered abilities", () => {
      const tree = parseOracleText("Whenever target creature dies, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Whenevertargetcreaturedies,drawacard.<EOF>");
    });
  });

  describe("Pillar 1: Value Expressions", () => {
    it("should parse paying X life cost", () => {
      const tree = parseOracleText("Pay X life: Draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("PayXlife:Drawacard.<EOF>");
    });

    it("should parse drawing twice X cards", () => {
      const tree = parseOracleText("Whenever another creature dies, draw twice X cards.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Wheneveranothercreaturedies,drawtwiceXcards.<EOF>");
    });

    it("should parse drawing half its power rounded up cards", () => {
      const tree = parseOracleText("Whenever another creature dies, draw half its power rounded up cards.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Wheneveranothercreaturedies,drawhalfitspowerroundedupcards.<EOF>");
    });

    it("should parse variable values like the number of cards in zones/battlefield", () => {
      const tree = parseOracleText("Whenever another creature dies, draw the number of Elves on the battlefield cards.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Wheneveranothercreaturedies,drawthenumberofElvesonthebattlefieldcards.<EOF>");
    });
  });

  describe("Pillar 2: Composable Card Filters", () => {
    it("should parse negative color qualifiers (nongreen)", () => {
      const tree = parseOracleText("Whenever target nongreen creature dies, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Whenevertargetnongreencreaturedies,drawacard.<EOF>");
    });

    it("should parse negative card type qualifiers (nonland)", () => {
      const tree = parseOracleText("Whenever a nonland permanent enters the battlefield, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Wheneveranonlandpermanententersthebattlefield,drawacard.<EOF>");
    });

    it("should parse status and supertype qualifiers together (legendary, tapped)", () => {
      const tree = parseOracleText("Whenever target tapped legendary artifact dies, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Whenevertargettappedlegendaryartifactdies,drawacard.<EOF>");
    });

    it("should parse relation qualifiers (another)", () => {
      const tree = parseOracleText("Whenever another creature dies, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Wheneveranothercreaturedies,drawacard.<EOF>");
    });

    it("should parse complex location specifications (in your graveyard)", () => {
      const tree = parseOracleText("Whenever target creature card in your graveyard dies, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Whenevertargetcreaturecardinyourgraveyarddies,drawacard.<EOF>");
    });
  });

  describe("Pillar 3: Conditionals and Choice Structures", () => {
    it("should parse choose-one abilities", () => {
      const tree = parseOracleText("Choose one — \n • Flying \n • Haste");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Chooseone—•Flying•Haste<EOF>");
    });

    it("should parse choose-one-or-more abilities with effects", () => {
      const tree = parseOracleText("Choose one or more — \n • Draw a card. \n • Destroy target creature.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Chooseoneormore—•Drawacard.•Destroytargetcreature.<EOF>");
    });

    it("should parse simple if conditions", () => {
      const tree = parseOracleText("If you control a creature, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Ifyoucontrolacreature,drawacard.<EOF>");
    });

    it("should parse unless conditions", () => {
      const tree = parseOracleText("Draw a card unless target player pays {2}.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Drawacardunlesstargetplayerpays{2}.<EOF>");
    });

    it("should parse if you do and otherwise clauses", () => {
      const tree = parseOracleText("If you do, destroy target creature. Otherwise, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Ifyoudo,destroytargetcreature.Otherwise,drawacard.<EOF>");
    });

    it("should parse turn and status conditions", () => {
      const tree = parseOracleText("If it is your turn, draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Ifitisyourturn,drawacard.<EOF>");
    });
  });

  describe("Pillar 4: Dynamic Durations and Continuous Effects", () => {
    it("should parse conditional static abilities with gets modifier", () => {
      const tree = parseOracleText("As long as you control a creature, target creature gets +1/+1 until end of turn.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Aslongasyoucontrolacreature,targetcreaturegets+1/+1untilendofturn.<EOF>");
    });

    it("should parse conditional static abilities with have keyword", () => {
      const tree = parseOracleText("As long as you control a creature, target creature has flying.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Aslongasyoucontrolacreature,targetcreaturehasflying.<EOF>");
    });

    it("should parse during turn durations", () => {
      const tree = parseOracleText("During your turn, target creature has haste.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Duringyourturn,targetcreaturehashaste.<EOF>");
    });

    it("should parse remains on battlefield conditions", () => {
      const tree = parseOracleText("As long as it remains on the battlefield, target creature has flying.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Aslongasitremainsonthebattlefield,targetcreaturehasflying.<EOF>");
    });
  });

  describe("New Features (Exile, Gains, Becomes, and Name Normalization)", () => {
    it("should parse exile effects", () => {
      const tree = parseOracleText("Exile target creature.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Exiletargetcreature.<EOF>");
    });

    it("should parse exile with location", () => {
      const tree = parseOracleText("Exile target creature card from your graveyard.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Exiletargetcreaturecardfromyourgraveyard.<EOF>");
    });

    it("should parse activated ability with exile cost", () => {
      const tree = parseOracleText("{T}, Exile this artifact: Draw a card.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("{T},Exilethisartifact:Drawacard.<EOF>");
    });

    it("should parse gaining life", () => {
      const tree = parseOracleText("You gain 5 life.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Yougain5life.<EOF>");
    });

    it("should parse gaining keyword abilities with duration", () => {
      const tree = parseOracleText("Target creature gains flying and haste until end of turn.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Targetcreaturegainsflyingandhasteuntilendofturn.<EOF>");
    });

    it("should parse becoming a creature with base P/T", () => {
      const tree = parseOracleText("target token you control becomes a green Bear creature with base power and toughness 4/4.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("targettokenyoucontrolbecomesagreenBearcreaturewithbasepowerandtoughness4/4.<EOF>");
    });

    it("should parse becoming a copy", () => {
      const tree = parseOracleText("target creature becomes a copy of target opponent.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("targetcreaturebecomesacopyoftargetopponent.<EOF>");
    });

    it("should parse multiple effects with then connector", () => {
      const tree = parseOracleText("Exile target creature you control, then return that card to the battlefield under its owner's control.");
      expect(tree).toBeDefined();
      expect(tree.text).toBe("Exiletargetcreatureyoucontrol,thenreturnthatcardtothebattlefieldunderitsowner'scontrol.<EOF>");
    });

    it("should normalize self-reference names and possessives correctly", () => {
      const norm1 = normalizeOracleText("Whenever Halsin, Emerald Archdruid enters, Halsin gets +1/+1.", "Halsin, Emerald Archdruid");
      expect(norm1).toBe("Whenever ~ enters, ~ gets +1/+1.");

      const norm2 = normalizeOracleText("Whenever Halsin's ability resolves, you gain 5 life.", "Halsin, Emerald Archdruid");
      expect(norm2).toBe("Whenever its ability resolves, you gain 5 life.");
    });

    it("should parse generalized casting triggers", () => {
      const tree1 = parseOracleText("Whenever you cast a noncreature spell, draw a card.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Wheneveryoucastanoncreaturespell,drawacard.<EOF>");

      const tree2 = parseOracleText("Whenever you cast an instant or sorcery spell, draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Wheneveryoucastaninstantorsorceryspell,drawacard.<EOF>");

      const tree3 = parseOracleText("Whenever you cast an Aura or Equipment spell, draw a card.");
      expect(tree3).toBeDefined();
      expect(tree3.text).toBe("WheneveryoucastanAuraorEquipmentspell,drawacard.<EOF>");

      const tree4 = parseOracleText("Whenever a player casts a black spell, draw a card.");
      expect(tree4).toBeDefined();
      expect(tree4.text).toBe("Wheneveraplayercastsablackspell,drawacard.<EOF>");
    });

    it("should parse property qualifiers", () => {
      const tree1 = parseOracleText("Destroy target creature with power 3 or greater.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Destroytargetcreaturewithpower3orgreater.<EOF>");

      const tree2 = parseOracleText("Destroy all artifact cards with mana value 4 or less.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Destroyallartifactcardswithmanavalue4orless.<EOF>");
    });

    it("should parse CDAs (characteristic-defining abilities)", () => {
      const tree1 = parseOracleText("its power is equal to the number of land cards in your graveyard.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("itspowerisequaltothenumberoflandcardsinyourgraveyard.<EOF>");

      const tree2 = parseOracleText("its power and toughness are each equal to the number of Islands you control.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("itspowerandtoughnessareeachequaltothenumberofIslandsyoucontrol.<EOF>");
    });

    it("should parse ability word prefixes", () => {
      const tree1 = parseOracleText("Landfall — Whenever a land you control enters, draw a card.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Landfall—Wheneveralandyoucontrolenters,drawacard.<EOF>");

      const tree2 = parseOracleText("Channel — Discard this card: Draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Channel—Discardthiscard:Drawacard.<EOF>");
    });

    it("should parse gain control effects", () => {
      const tree1 = parseOracleText("Gain control of target creature until end of turn.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Gaincontroloftargetcreatureuntilendofturn.<EOF>");

      const tree2 = parseOracleText("Gain control of target Aura.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("GaincontroloftargetAura.<EOF>");
    });

    it("should parse pronoun and relative selectors", () => {
      const tree1 = parseOracleText("Exile it.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Exileit.<EOF>");

      const tree2 = parseOracleText("Destroy them.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Destroythem.<EOF>");

      const tree3 = parseOracleText("return that card to its owner's hand.");
      expect(tree3).toBeDefined();
      expect(tree3.text).toBe("returnthatcardtoitsowner'shand.<EOF>");

      const tree4 = parseOracleText("return target creature to their owners' hands.");
      expect(tree4).toBeDefined();
      expect(tree4.text).toBe("returntargetcreaturetotheirowners'hands.<EOF>");
    });

    it("should parse multi-sentence abilities", () => {
      const tree1 = parseOracleText("Whenever this creature enters, draw a card. Then destroy target creature.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Wheneverthiscreatureenters,drawacard.Thendestroytargetcreature.<EOF>");

      const tree2 = parseOracleText("Whenever you cast a noncreature spell, draw a card. Otherwise, destroy target creature.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Wheneveryoucastanoncreaturespell,drawacard.Otherwise,destroytargetcreature.<EOF>");
    });

    it("should parse token clash name words", () => {
      const tree1 = parseOracleText("Stunning Strike — {T}: Tap target creature.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("StunningStrike—{T}:Taptargetcreature.<EOF>");

      const tree2 = parseOracleText("Beacon of Hope — Whenever another creature dies, draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("BeaconofHope—Wheneveranothercreaturedies,drawacard.<EOF>");
    });

    it("should parse tap and untap verbs", () => {
      const tree1 = parseOracleText("Tap target creature.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Taptargetcreature.<EOF>");

      const tree2 = parseOracleText("Untap it.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Untapit.<EOF>");
    });

    it("should parse put effects (counters and zones)", () => {
      const tree1 = parseOracleText("put a +1/+1 counter on target creature.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("puta+1/+1counterontargetcreature.<EOF>");

      const tree2 = parseOracleText("put that card onto the battlefield tapped.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("putthatcardontothebattlefieldtapped.<EOF>");

      const tree3 = parseOracleText("put it into your graveyard.");
      expect(tree3).toBeDefined();
      expect(tree3.text).toBe("putitintoyourgraveyard.<EOF>");
    });

    it("should parse search and shuffle effects", () => {
      const tree1 = parseOracleText("search your library for a basic land card, put that card onto the battlefield tapped, then shuffle.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("searchyourlibraryforabasiclandcard,putthatcardontothebattlefieldtapped,thenshuffle.<EOF>");

      const tree2 = parseOracleText("shuffle your library.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("shuffleyourlibrary.<EOF>");
    });

    it("should parse advanced token creation", () => {
      const tree1 = parseOracleText("create a 1/1 white Soldier creature token with flying.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("createa1/1whiteSoldiercreaturetokenwithflying.<EOF>");
    });

    it("should parse keyword lists", () => {
      const tree1 = parseOracleText("Flying, haste");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Flying,haste<EOF>");

      const tree2 = parseOracleText("Flying, vigilance, and haste");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Flying,vigilance,andhaste<EOF>");
    });

    it("should parse compound static effects", () => {
      const tree1 = parseOracleText("As long as you control a creature, target creature gets +1/+1 and has flying.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Aslongasyoucontrolacreature,targetcreaturegets+1/+1andhasflying.<EOF>");

      const tree2 = parseOracleText("Other Cats you control get +1/+1 and have haste.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("OtherCatsyoucontrolget+1/+1andhavehaste.<EOF>");
    });

    it("should parse expanded action costs", () => {
      const tree1 = parseOracleText("Tap an untapped Ally you control: draw a card.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("TapanuntappedAllyyoucontrol:drawacard.<EOF>");

      const tree2 = parseOracleText("Return two Islands you control to their owner's hand: draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("ReturntwoIslandsyoucontroltotheirowner'shand:drawacard.<EOF>");
    });

    it("should parse combat, attack, and block triggers", () => {
      const tree1 = parseOracleText("Whenever this creature attacks, draw a card.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Wheneverthiscreatureattacks,drawacard.<EOF>");

      const tree2 = parseOracleText("Whenever a creature attacks you, draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Wheneveracreatureattacksyou,drawacard.<EOF>");

      const tree3 = parseOracleText("At the beginning of combat on your turn, draw a card.");
      expect(tree3).toBeDefined();
      expect(tree3.text).toBe("Atthebeginningofcombatonyourturn,drawacard.<EOF>");

      const tree4 = parseOracleText("Whenever this creature blocks, draw a card.");
      expect(tree4).toBeDefined();
      expect(tree4.text).toBe("Wheneverthiscreatureblocks,drawacard.<EOF>");

      const tree5 = parseOracleText("Whenever this creature becomes blocked, draw a card.");
      expect(tree5).toBeDefined();
      expect(tree5.text).toBe("Wheneverthiscreaturebecomesblocked,drawacard.<EOF>");
    });

    it("should parse unified phase and turn triggers", () => {
      const tree1 = parseOracleText("At the beginning of your upkeep, draw a card.");
      expect(tree1).toBeDefined();
      expect(tree1.text).toBe("Atthebeginningofyourupkeep,drawacard.<EOF>");

      const tree2 = parseOracleText("At the beginning of each player's upkeep, draw a card.");
      expect(tree2).toBeDefined();
      expect(tree2.text).toBe("Atthebeginningofeachplayer'supkeep,drawacard.<EOF>");

      const tree3 = parseOracleText("At the beginning of the untap step on your turn, draw a card.");
      expect(tree3).toBeDefined();
      expect(tree3.text).toBe("Atthebeginningoftheuntapsteponyourturn,drawacard.<EOF>");

      const tree4 = parseOracleText("At the beginning of the draw step on your turn, draw a card.");
      expect(tree4).toBeDefined();
      expect(tree4.text).toBe("Atthebeginningofthedrawsteponyourturn,drawacard.<EOF>");

      const tree5 = parseOracleText("At the beginning of each end step, draw a card.");
      expect(tree5).toBeDefined();
      expect(tree5.text).toBe("Atthebeginningofeachendstep,drawacard.<EOF>");

      const tree6 = parseOracleText("At the beginning of your end step, draw a card.");
      expect(tree6).toBeDefined();
      expect(tree6.text).toBe("Atthebeginningofyourendstep,drawacard.<EOF>");

      const tree7 = parseOracleText("At the beginning of your first main phase, draw a card.");
      expect(tree7).toBeDefined();
      expect(tree7.text).toBe("Atthebeginningofyourfirstmainphase,drawacard.<EOF>");

      const tree8 = parseOracleText("At the beginning of the precombat main phase on your turn, draw a card.");
      expect(tree8).toBeDefined();
      expect(tree8.text).toBe("Atthebeginningoftheprecombatmainphaseonyourturn,drawacard.<EOF>");
    });
  });

  describe("Failure Resolutions & Extensions", () => {
    it("should parse mana production abilities", () => {
      const tree1 = parseOracleText("{T}: Add {C}.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Add {R} or {G}.");
      expect(tree2).toBeDefined();

      const tree3 = parseOracleText("Add one mana of any color.");
      expect(tree3).toBeDefined();
    });

    it("should parse enters or leaves triggers", () => {
      const tree = parseOracleText("When this creature enters or leaves the battlefield, draw a card.");
      expect(tree).toBeDefined();
    });

    it("should parse replacement and cost effects starting with As", () => {
      const tree1 = parseOracleText("As this Aura enters, choose a color.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("As an additional cost to cast this spell, sacrifice a creature.");
      expect(tree2).toBeDefined();
    });

    it("should parse quoted abilities", () => {
      const tree = parseOracleText("lands you control gain \"{T}: Add one mana of any color.\"");
      expect(tree).toBeDefined();
    });

    it("should parse optional durations and extended conditions", () => {
      const tree1 = parseOracleText("this creature has defender as long as it is a Wall.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("this creature gets +2/+1 as long as an opponent has 10 or less life.");
      expect(tree2).toBeDefined();
    });
  });

  describe("Layered Keywords & Multipliers", () => {
    it("should parse new simple and compound keywords", () => {
      const tree1 = parseOracleText("flash, shroud");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("hexproof, indestructible");
      expect(tree2).toBeDefined();
    });

    it("should parse parametric keywords", () => {
      const tree1 = parseOracleText("protection from green");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("protection from red and from blue");
      expect(tree2).toBeDefined();

      const tree3 = parseOracleText("protection from everything");
      expect(tree3).toBeDefined();

      const tree4 = parseOracleText("hexproof from planeswalkers");
      expect(tree4).toBeDefined();

      const tree5 = parseOracleText("Enchant creature");
      expect(tree5).toBeDefined();
    });

    it("should parse combined triggers", () => {
      const tree1 = parseOracleText("Whenever this creature enters or attacks, draw a card.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Whenever this creature enters the battlefield or attacks, draw a card.");
      expect(tree2).toBeDefined();
    });

    it("should parse action multipliers", () => {
      const tree1 = parseOracleText("For each player, choose friend or foe.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Draw a card for each tapped creature target opponent controls.");
      expect(tree2).toBeDefined();
    });

    it("should parse wildcard locations", () => {
      const tree = parseOracleText("Exile target card from a graveyard.");
      expect(tree).toBeDefined();
    });
  });

  describe("Planeswalker Loyalty Abilities", () => {
    it("should parse positive loyalty cost abilities", () => {
      const tree = parseOracleText("+1: Create three 1/1 white Soldier creature tokens.");
      expect(tree).toBeDefined();
    });

    it("should parse negative loyalty cost abilities", () => {
      const tree = parseOracleText("−3: Destroy all creatures.");
      expect(tree).toBeDefined();
    });

    it("should parse zero loyalty cost abilities", () => {
      const tree = parseOracleText("0: Create a 1/1 Devil token.");
      expect(tree).toBeDefined();
    });

    it("should parse variable loyalty cost abilities", () => {
      const tree1 = parseOracleText("+X: draw a card.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("−X: draw a card.");
      expect(tree2).toBeDefined();
    });
  });

  describe("Spell Targeting & Relative Clauses", () => {
    it("should parse Heroic and Aura casting triggers that target", () => {
      const tree1 = parseOracleText("Whenever you cast a spell that targets this creature, this creature gets +2/+0 until end of turn.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Whenever you cast an Aura spell that targets this creature, create a 3/3 green Beast creature token.");
      expect(tree2).toBeDefined();
    });

    it("should parse casting triggers with relative clauses specifying targets count", () => {
      const tree1 = parseOracleText("Whenever you cast a spell that targets one or more creatures, those creatures gain flying.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Whenever you cast an instant or sorcery spell that targets only a single creature you control, copy that spell.");
      expect(tree2).toBeDefined();
    });

    it("should parse activation triggers targeting specific types", () => {
      const tree1 = parseOracleText("Whenever you activate an ability that targets a creature or player, copy that ability.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Whenever you activate a loyalty ability of a Chandra planeswalker, this creature deals 1 damage to each opponent.");
      expect(tree2).toBeDefined();
    });

    it("should parse triggers when permanents become target of spells/abilities", () => {
      const tree1 = parseOracleText("When this creature becomes the target of a spell, sacrifice it.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Whenever this creature becomes the target of a spell or ability, sacrifice it.");
      expect(tree2).toBeDefined();

      const tree3 = parseOracleText("Whenever this creature becomes the target of a spell an opponent controls, draw a card.");
      expect(tree3).toBeDefined();
    });

    it("should parse targeting relative clauses in targets and search effects", () => {
      const tree1 = parseOracleText("Counter target instant or sorcery spell that targets you.");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Counter target spell that targets an enchantment.");
      expect(tree2).toBeDefined();

      const tree3 = parseOracleText("Reveal two cards from your hand that share a color.");
      expect(tree3).toBeDefined();
    });
  });

  describe("Gains Effects & Bulleted Choose One Titled Choices", () => {
    it("should parse gains static and continuous effects", () => {
      const tree1 = parseOracleText("lands you control gain \"{T}: Add one mana of any color.\"");
      expect(tree1).toBeDefined();

      const tree2 = parseOracleText("Until end of turn, target creature you control gains indestructible and \"Whenever this creature attacks, draw a card.\"");
      expect(tree2).toBeDefined();
    });

    it("should parse titled bullet choice options", () => {
      const tree = parseOracleText("Choose one — \n • Cure Wounds — You gain 2 life. \n • Dispel Magic — Destroy target enchantment.");
      expect(tree).toBeDefined();
    });

    it("should parse bullet choice options containing ampersands", () => {
      const tree = parseOracleText("Choose one — \n • Hearth & Home — Exile target creature. \n • Hearth & Fire — ~ deals 2 damage to target player.");
      expect(tree).toBeDefined();
    });

    it("should parse triggered choice abilities", () => {
      const tree = parseOracleText("When this creature enters, choose one — \n • Cure Wounds — You gain 2 life. \n • Dispel Magic — Destroy target enchantment.");
      expect(tree).toBeDefined();
    });
  });
});
