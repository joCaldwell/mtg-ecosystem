// packages/oracle-parser/tests/parser.test.ts
import { describe, it, expect } from "vitest";
import { parseOracleText } from "../src/index.js";

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
});
