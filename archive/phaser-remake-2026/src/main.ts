import { createGame } from "@/game/createGame";
import { mountConsole } from "@/game/ui/TermOverlay";
import "./style.css";

const game = createGame("app");
mountConsole(game);
