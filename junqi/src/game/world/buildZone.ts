import type { ZoneId } from "@/game/types";
import { generateBurial } from "@/game/world/generateBurial";
import { generateCellar } from "@/game/world/generateCellar";
import { generateHouse } from "@/game/world/generateHouse";
import { generateMine } from "@/game/world/generateMine";
import { generateButterfly, generateFactory, generatePipes, generateSchool } from "@/game/world/generateDungeons";
import { generateAbandoned, generateGraveyard, generateMuseum } from "@/game/world/generatePlaces";
import { generateTownYard } from "@/game/world/generateTownYard";
import { assertRequired } from "@/game/world/stamp";
import type { ZoneBlue } from "@/game/world/zoneTypes";

export function buildZone(zone: ZoneId, seed: number): ZoneBlue {
  const blue = makeZone(zone, seed);
  assertRequired(zone, blue);
  return blue;
}

function makeZone(zone: ZoneId, seed: number): ZoneBlue {
  switch (zone) {
    case "house":
      return generateHouse();
    case "dungeon":
      return generateCellar(seed);
    case "burial":
      return generateBurial(seed);
    case "mine":
      return generateMine(seed);
    case "abandoned":
      return generateAbandoned(seed);
    case "museum":
      return generateMuseum(seed);
    case "graveyard":
      return generateGraveyard(seed);
    case "factory":
      return generateFactory();
    case "school":
      return generateSchool();
    case "butterfly":
      return generateButterfly();
    case "pipes":
      return generatePipes();
    default:
      return generateTownYard(seed);
  }
}
