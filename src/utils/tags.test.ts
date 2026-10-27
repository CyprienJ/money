import { describe, expect, it } from "vitest";
import { addTag, cleanTagName } from "./tags";

describe("cleanTagName", () => {
  it("trims and collapses spaces", () => {
    expect(cleanTagName("  Week-end   à Lyon ")).toBe("Week-end à Lyon");
  });
});

describe("addTag", () => {
  it("adds a cleaned name", () => {
    expect(addTag(["Cadeaux"], " vacances ")).toEqual(["Cadeaux", "vacances"]);
  });

  it("ignores empty names and repeats, whatever the case", () => {
    const tags = ["Cadeaux"];
    expect(addTag(tags, "  ")).toBe(tags);
    expect(addTag(tags, "CADEAUX")).toBe(tags);
  });

  it("takes the spelling of an existing tag", () => {
    expect(addTag([], "VACANCES", ["Cadeaux", "Vacances"])).toEqual(["Vacances"]);
  });
});
