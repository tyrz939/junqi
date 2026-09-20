import Phaser from "phaser";
import {
  CARRY_ENERGY_DRAIN,
  HOUR_SECONDS,
  MELEE_ENERGY_RESTORE,
  PATH_CELL,
  TILE_CHUNK_CELLS,
  UNIT_RADIUS,
} from "@/game/constants";
import { Unit } from "@/game/entities/Unit";
import { runActions, type ActionWorld } from "@/game/systems/actions";
import { tickAi, type AiActor } from "@/game/systems/ai";
import { getEnemy, getSpell, keyForLock } from "@/game/systems/catalog";
import {
  metres,
  meleeDamage,
  schoolDamage,
  spellErrorText,
  tryCast,
} from "@/game/systems/combat";
import { pickNode } from "@/game/systems/dialogue";
import { inventoryAdd, inventoryQuantity, inventoryRemove } from "@/game/systems/inventory";
import { Lighting } from "@/game/systems/lighting";
import { chunkInRing, inLoadRing, sameBlock, worldBlock, type Block } from "@/game/systems/loadRing";
import { hasLos } from "@/game/systems/los";
import { rollLoot } from "@/game/systems/loot";
import { session, type WorldInspect } from "@/game/systems/session";
import { applyStatus, isRooted, speedMul } from "@/game/systems/status";
import { surviveSpeed, tickSurvive } from "@/game/systems/survive";
import { inTriggerRect } from "@/game/systems/triggers";
import { angleBetween, angleDelta, faceTexture, spellVfx, type SpellVfx } from "@/game/systems/vfx";
import { ActionBar } from "@/game/ui/ActionBar";
import { DialogueBox } from "@/game/ui/DialogueBox";
import { Gui } from "@/game/ui/Gui";
import { Hud } from "@/game/ui/Hud";
import { Layers } from "@/game/ui/layers";
import { ZONE_TITLE } from "@/game/ui/zoneTitles";
import { buildZone } from "@/game/world/buildZone";
import { Grid } from "@/game/world/Grid";
import type { PropSpec, ZoneBlue } from "@/game/world/zoneTypes";
import type { DamageType, ZoneId } from "@/game/types";

type Actor = {
  unit: Unit;
  sprite: Phaser.GameObjects.Image;
  kind: string;
  leashX: number;
  leashY: number;
  path: { x: number; y: number }[];
  patrol?: { x: number; y: number }[];
  patrolI: number;
};

type Prop = PropSpec & { sprite: Phaser.GameObjects.Image; used?: boolean };

type Bolt = {
  sprite: Phaser.GameObjects.Image;
  x: number;
  y: number;
  vx: number;
  vy: number;
  life: number;
  school: DamageType;
  casterId: string;
  radius: number;
  spellId: string;
};

export class PlayScene extends Phaser.Scene {
  private blue!: ZoneBlue;
  private grid!: Grid;
  private layers!: Layers;
  private player!: Actor;
  private actors: Actor[] = [];
  private props: Prop[] = [];
  private bolts: Bolt[] = [];
  private keys!: Record<string, Phaser.Input.Keyboard.Key>;
  private hud!: Hud;
  private gui!: Gui;
  private bar!: ActionBar;
  private lighting!: Lighting;
  private talk!: DialogueBox;
  private reticle!: Phaser.GameObjects.Image;
  private showGrid = false;
  private gridGfx?: Phaser.GameObjects.Graphics;
  private tileChunks = new Map<string, Phaser.GameObjects.RenderTexture>();
  private lastBlock: Block | null = null;
  private asleep = new Set<string>();
  private insideTriggers = new Set<string>();
  private extraLights: { x: number; y: number; radius: number }[] = [];
  private pushWait = 0;
  private useMark!: Phaser.GameObjects.Graphics;
  private highlighted: Prop | null = null;
  private zoneBusy = false;

  constructor() {
    super("play");
  }

  create(): void {
    if (!session.started) session.newRun();
    if (!session.visited.includes(session.zone)) session.visited.push(session.zone);
    this.layers = new Layers(this);
    this.blue = buildZone(session.zone, session.seed);
    this.grid = this.blue.grid;
    this.tileChunks.clear();
    this.asleep.clear();
    this.insideTriggers.clear();
    this.extraLights = [];
    this.lastBlock = null;
    this.actors = [];
    this.bolts = [];
    this.props = this.blue.props.map((spec) => {
      const sprite = this.worldImage(spec.x, spec.y, spec.texture);
      if (session.flag(`used:${spec.id}`)) sprite.setAlpha(0.45);
      return { ...spec, sprite, used: session.flag(`used:${spec.id}`) };
    });
    for (const spec of this.blue.enemies) {
      if (session.dead.has(spec.id)) continue;
      const actor = this.spawnEnemy(spec);
      actor.patrol = spec.patrol;
      this.actors.push(actor);
    }
    this.applyDrainedFloods();
    this.applyZoneSnap();
    this.applyPropSolids();
    if (session.flag("burial_unlocked")) this.setPropLock("boss_gate", false);
    for (const prop of this.props) {
      if (prop.kind === "broken" && session.flag(`used:${prop.id}`)) {
        prop.kind = "bench";
        prop.sprite.setTexture("crate");
      }
    }

    const start = this.playStart();
    this.player = {
      unit: session.player,
      sprite: this.worldImage(start.x, start.y, faceTexture("player", session.player.facing)),
      kind: "player",
      leashX: start.x,
      leashY: start.y,
      path: [],
      patrolI: 0,
    };
    this.reticle = this.worldImage(0, 0, "reticle").setVisible(false).setDepth(400);
    this.tweens.add({ targets: this.reticle, alpha: 0.4, duration: 480, yoyo: true, repeat: -1 });
    this.dressWorld();

    const worldW = this.grid.cols * PATH_CELL;
    const worldH = this.grid.rows * PATH_CELL;
    this.layers.worldCam.setBounds(0, 0, worldW, worldH);
    this.layers.worldCam.centerOn(this.player.sprite.x, this.player.sprite.y);
    this.layers.worldCam.startFollow(this.player.sprite, true, 0.14, 0.14);
    this.lighting = new Lighting(this, this.layers);
    this.hud = new Hud(this, this.layers);
    this.gui = new Gui(this, this.layers);
    this.bar = new ActionBar(this, this.layers);
    this.talk = new DialogueBox(this, this.layers);
    this.useMark = this.layers.addWorld(this.add.graphics().setDepth(50));
    this.bindKeys();
    this.hud.fadeArrive();
    this.input.mouse?.disableContextMenu();
    this.input.on("pointerdown", (p: Phaser.Input.Pointer) => this.onPointer(p));
    this.input.on("pointerup", (p: Phaser.Input.Pointer) => this.onPointerUp(p));
    this.input.on(
      "wheel",
      (pointer: Phaser.Input.Pointer, _over: unknown, _dx: number, dy: number, _dz?: number, ev?: WheelEvent) => {
        if (!session.guiOpen || this.talk.open) return;
        ev?.preventDefault();
        this.gui.wheel(dy, pointer.x, pointer.y);
      },
    );
    session.inspectWorld = () => this.census();
    this.events.once(Phaser.Scenes.Events.SHUTDOWN, () => {
      if (session.inspectWorld) session.inspectWorld = null;
    });
    this.syncLoadRing();
    this.noteZone();
  }

  update(_t: number, delta: number): void {
    const dt = delta / 1000;
    if (session.wantRestart || session.wantQuit) {
      session.toTitle(session.wantRestart);
      this.scene.start("title");
      return;
    }
    if (this.consume("ESC") && !session.terminalOpen) {
      this.bar.cancelPress();
      const busy = Boolean(session.held) || this.gui.isPressing();
      this.gui.cancelHeld();
      if (!busy) {
        if (session.guiOpen) session.guiOpen = false;
        else session.paused = !session.paused;
      }
    }
    if (!session.terminalOpen && (this.consume("TAB") || this.consume("I"))) {
      this.bar.cancelPress();
      this.gui.cancelHeld();
      session.guiOpen = !session.guiOpen;
    }
    if (session.guiOpen && !session.terminalOpen) {
      if (this.consume("left")) this.cycleWindow(-1);
      if (this.consume("right")) this.cycleWindow(1);
      if (this.consume("Q")) session.window = "quest";
      if (this.consume("K")) session.window = "spellbook";
      if (this.consume("M")) session.window = "map";
    }
    if (session.pendingZone && !this.zoneBusy) {
      const dest = session.pendingZone;
      session.pendingZone = null;
      this.beginLeave(() => {
        this.dropCarry();
        this.stashZone();
        session.zone = dest;
        session.entry = { x: 0, y: 0 };
        session.marks[dest] = undefined;
        this.scene.restart();
      });
    }
    if (session.wantSave || (!session.terminalOpen && this.consume("F5"))) {
      session.wantSave = false;
      this.writeSlot();
    }
    if (!session.terminalOpen && this.consume("F3")) {
      this.showGrid = !this.showGrid;
      this.drawDebugGrid();
    }
    if (!session.terminalOpen && this.consume("F2")) session.debug = !session.debug;

    if (this.talk.open) {
      session.talking = true;
      if (!session.terminalOpen) {
        if (this.consume("E") || this.consume("space")) this.talk.advance().forEach((n) => this.hud.toast(n));
        if (this.consume("ONE")) this.talk.choose(0, this.worldCtx()).forEach((n) => this.hud.toast(n));
        if (this.consume("TWO")) this.talk.choose(1, this.worldCtx()).forEach((n) => this.hud.toast(n));
      }
      if (!this.talk.open) session.talking = false;
    } else {
      session.talking = false;
    }

    if (!session.worldFrozen() && !this.zoneBusy) {
      session.time = (session.time + dt / HOUR_SECONDS) % 24;
      this.syncLoadRing();
      this.player.unit.tickTimers(dt);
      for (const a of this.actors) {
        if (!this.asleep.has(a.unit.id)) a.unit.tickTimers(dt);
      }
      this.movePlayer(dt);
      this.actions();
      this.autoAttack();
      this.ai(dt);
      this.tickBolts(dt);
      this.flush(this.player);
      for (const a of this.actors) {
        if (!this.asleep.has(a.unit.id) || a.unit.incoming.length > 0) this.flush(a);
      }
      this.respawnTick();
      this.followCarry();
      this.tickTriggers();
      const nearFire = this.nearKind("campfire", 28);
      const note = tickSurvive(
        this.player.unit,
        dt,
        session.time,
        session.zone,
        this.player.sprite.x,
        this.player.sprite.y,
        nearFire,
      );
      if (note) this.hud.toast(note);
    }

    session.quests.syncAcquire(session.player);
    session.flags.near_bench = this.nearKind("bench", 20) ? 1 : 0;
    this.grid.occupy(this.player.unit.id, this.player.sprite.x, this.player.sprite.y);
    for (const a of this.actors) {
      if (a.unit.alive && !this.asleep.has(a.unit.id)) this.grid.occupy(a.unit.id, a.sprite.x, a.sprite.y);
      else this.grid.clearOccupy(a.unit.id);
    }
    this.face(this.player);
    for (const a of this.actors) {
      this.face(a);
      a.sprite.setVisible(a.unit.alive && !this.asleep.has(a.unit.id));
    }
    const locked = this.currentTarget();
    this.reticle.setVisible(Boolean(locked?.unit.alive));
    if (locked) this.reticle.setPosition(locked.sprite.x, locked.sprite.y).setDepth(locked.sprite.y + 8);

    this.lighting.draw(
      [
        { x: this.player.sprite.x, y: this.player.sprite.y, radius: 46 },
        ...this.blue.lights.filter((l) => inLoadRing(l.x, l.y, this.player.sprite.x, this.player.sprite.y)),
        ...this.extraLights,
      ],
      session.time,
    );
    this.tickUseHighlight();
    const ptr = this.input.activePointer;
    const lockedDist = locked ? metres(this.dist(locked.sprite)) : null;
    this.bar.update(ptr.x, ptr.y, session.player, lockedDist);
    this.hud.update(
      dt,
      session.player,
      session.beats,
      session.seed,
      locked?.unit ?? null,
      `${session.zone} actors ${this.actors.filter((a) => a.unit.alive).length} sleep ${this.asleep.size} gcd ${session.player.gcd.toFixed(2)} carry ${session.carrying ?? "-"}`,
    );
    this.gui.update(ptr.x, ptr.y, this.grid, this.player.sprite.x, this.player.sprite.y);
  }

  private onPointer(p: Phaser.Input.Pointer): void {
    const right = p.rightButtonDown() || p.button === 2;
    if (this.talk.open) {
      this.talk.click(p.x, p.y, this.worldCtx()).forEach((n) => this.hud.toast(n));
      if (!this.talk.open) session.talking = false;
      return;
    }
    if (session.guiOpen) {
      if (this.bar.hit(p.x, p.y)) {
        const result = this.bar.press(p.x, p.y, right);
        if (result.grab != null && session.actionBar[result.grab]) this.gui.holdFrom("bar", result.grab);
        this.handleGuiMsg(result.msg ?? "");
        return;
      }
      this.handleGuiMsg(this.gui.press(p.x, p.y, right) ?? "");
      return;
    }
    if (session.worldFrozen()) return;
    if (this.bar.hit(p.x, p.y)) {
      this.handleGuiMsg(this.bar.click(p.x, p.y, right) ?? "");
      return;
    }
    if (!right) this.pickTarget(p);
  }

  private onPointerUp(p: Phaser.Input.Pointer): void {
    if (!session.guiOpen || this.talk.open) return;
    if (p.button === 2) return;
    if (this.bar.hit(p.x, p.y)) {
      this.handleGuiMsg(this.bar.release(p.x, p.y) ?? "");
      this.gui.droppedOnBar();
      return;
    }
    this.bar.cancelPress();
    this.handleGuiMsg(this.gui.release(p.x, p.y) ?? "");
  }

  private pickTarget(p: Phaser.Input.Pointer): void {
    const world = this.layers.worldCam.getWorldPoint(p.x, p.y);
    let best: Actor | null = null;
    let bestD = 12;
    for (const a of this.actors) {
      if (!a.unit.alive || this.asleep.has(a.unit.id)) continue;
      const d = Phaser.Math.Distance.Between(world.x, world.y, a.sprite.x, a.sprite.y);
      if (d < bestD) {
        best = a;
        bestD = d;
      }
    }
    this.player.unit.currentTargetId = best?.unit.id ?? null;
  }

  private currentTarget(): Actor | null {
    const id = this.player.unit.currentTargetId;
    if (!id) return null;
    return this.actors.find((a) => a.unit.id === id && a.unit.alive) ?? null;
  }

  private bindKeys(): void {
    const kb = this.input.keyboard;
    if (!kb) throw new Error("keyboard");
    const codes: Record<string, number> = {
      up: Phaser.Input.Keyboard.KeyCodes.UP,
      down: Phaser.Input.Keyboard.KeyCodes.DOWN,
      left: Phaser.Input.Keyboard.KeyCodes.LEFT,
      right: Phaser.Input.Keyboard.KeyCodes.RIGHT,
      W: Phaser.Input.Keyboard.KeyCodes.W,
      A: Phaser.Input.Keyboard.KeyCodes.A,
      S: Phaser.Input.Keyboard.KeyCodes.S,
      D: Phaser.Input.Keyboard.KeyCodes.D,
      E: Phaser.Input.Keyboard.KeyCodes.E,
      Q: Phaser.Input.Keyboard.KeyCodes.Q,
      K: Phaser.Input.Keyboard.KeyCodes.K,
      M: Phaser.Input.Keyboard.KeyCodes.M,
      I: Phaser.Input.Keyboard.KeyCodes.I,
      TAB: Phaser.Input.Keyboard.KeyCodes.TAB,
      ESC: Phaser.Input.Keyboard.KeyCodes.ESC,
      SHIFT: Phaser.Input.Keyboard.KeyCodes.SHIFT,
      space: Phaser.Input.Keyboard.KeyCodes.SPACE,
      F2: Phaser.Input.Keyboard.KeyCodes.F2,
      F3: Phaser.Input.Keyboard.KeyCodes.F3,
      F5: Phaser.Input.Keyboard.KeyCodes.F5,
      ONE: Phaser.Input.Keyboard.KeyCodes.ONE,
      TWO: Phaser.Input.Keyboard.KeyCodes.TWO,
      THREE: Phaser.Input.Keyboard.KeyCodes.THREE,
      FOUR: Phaser.Input.Keyboard.KeyCodes.FOUR,
      FIVE: Phaser.Input.Keyboard.KeyCodes.FIVE,
      SIX: Phaser.Input.Keyboard.KeyCodes.SIX,
      SEVEN: Phaser.Input.Keyboard.KeyCodes.SEVEN,
      EIGHT: Phaser.Input.Keyboard.KeyCodes.EIGHT,
    };
    this.keys = kb.addKeys(codes) as Record<string, Phaser.Input.Keyboard.Key>;
    if (session.terminalOpen) {
      kb.enabled = false;
      kb.disableGlobalCapture();
      const manager = this.game.input.keyboard;
      if (manager) {
        manager.enabled = false;
        manager.preventDefault = false;
      }
    }
  }

  private consume(name: string): boolean {
    return Phaser.Input.Keyboard.JustDown(this.keys[name]);
  }

  private cycleWindow(dir: number): void {
    const order = ["inventory", "spellbook", "quest", "map"] as const;
    const i = (order.indexOf(session.window) + dir + order.length) % order.length;
    session.window = order[i];
  }

  private beginLeave(go: () => void): void {
    if (this.zoneBusy) return;
    this.zoneBusy = true;
    this.hud.fadeDepart(go);
  }

  private tickUseHighlight(): void {
    const show =
      this.player.unit.alive &&
      !session.carrying &&
      !session.worldFrozen() &&
      !this.zoneBusy;
    const prop = show ? this.closestProp(28) : null;
    if (this.highlighted !== prop) {
      if (this.highlighted) this.highlighted.sprite.clearTint();
      this.highlighted = prop;
    }
    this.useMark.clear();
    if (!prop) {
      this.hud.setUse(null);
      return;
    }
    prop.sprite.setTint(0xffe8a0);
    const x = prop.sprite.x;
    const y = prop.sprite.y + 7;
    this.useMark.setDepth(prop.sprite.y + 8);
    this.useMark.fillStyle(0xf4e080, 0.95);
    this.useMark.fillTriangle(x, y - 5, x - 5, y, x + 5, y);
    this.useMark.fillTriangle(x, y + 5, x - 5, y, x + 5, y);
    const screen = this.layers.worldToScreen(prop.sprite.x, prop.sprite.y - 10);
    this.hud.setUse(screen);
  }

  private dressWorld(): void {
    for (const prop of this.props) {
      if (
        prop.texture !== "torch" &&
        prop.texture !== "orb" &&
        prop.texture !== "orb_ice" &&
        prop.texture !== "shrine" &&
        prop.texture !== "campfire"
      ) {
        continue;
      }
      this.tweens.add({
        targets: prop.sprite,
        alpha: 0.7,
        duration: 340 + ((prop.x * 17) % 200),
        yoyo: true,
        repeat: -1,
      });
    }
  }

  private drawChunk(ccx: number, ccy: number): Phaser.GameObjects.RenderTexture {
    const w = TILE_CHUNK_CELLS * PATH_CELL;
    const h = TILE_CHUNK_CELLS * PATH_CELL;
    const rt = this.add.renderTexture(ccx * w, ccy * h, w, h).setOrigin(0).setDepth(0);
    const x0 = ccx * TILE_CHUNK_CELLS;
    const y0 = ccy * TILE_CHUNK_CELLS;
    for (let cy = y0; cy < Math.min(y0 + TILE_CHUNK_CELLS, this.grid.rows); cy++) {
      for (let cx = x0; cx < Math.min(x0 + TILE_CHUNK_CELLS, this.grid.cols); cx++) {
        const tile = this.grid.get(cx, cy);
        const key = tile === "grass" && (cx + cy) % 2 === 1 ? "grass_alt" : tile;
        rt.draw(key, (cx - x0) * PATH_CELL, (cy - y0) * PATH_CELL);
      }
    }
    return this.layers.addWorld(rt);
  }

  private worldImage(x: number, y: number, key: string): Phaser.GameObjects.Image {
    return this.layers.addWorld(this.add.image(x, y, key).setDepth(y));
  }

  private spawnEnemy(spec: { id: string; kind: string; x: number; y: number; respawn?: number }): Actor {
    const def = getEnemy(spec.kind);
    const unit = new Unit({
      id: spec.id,
      name: def.name,
      faction: def.faction,
      class: def.class,
      controller: "ai",
      strength: def.strength,
      spirit: def.spirit,
      walkSpd: def.walkSpd,
      runSpd: def.runSpd,
      spells: def.spells,
    });
    unit.enemyKind = spec.kind;
    unit.aggroRange = def.aggroRange;
    unit.leashRange = def.leashRange;
    unit.resist = def.resist ?? {};
    unit.respawn = spec.respawn ?? def.respawn ?? 0;
    unit.questUnit = Boolean(def.questUnit);
    const sheet = def.sprite ?? spec.kind;
    const sprite = this.worldImage(spec.x, spec.y, faceTexture(sheet, 270));
    if (def.scale) sprite.setScale(def.scale);
    return { unit, sprite, kind: spec.kind, leashX: spec.x, leashY: spec.y, path: [], patrolI: 0 };
  }

  private face(actor: Actor): void {
    const sheet =
      actor.kind === "player" || actor.kind === "dog" ? actor.kind : (getEnemy(actor.kind).sprite ?? actor.kind);
    const key = faceTexture(sheet, actor.unit.facing);
    if (this.textures.exists(key)) actor.sprite.setTexture(key);
    actor.sprite.setDepth(actor.sprite.y);
  }

  private movePlayer(dt: number): void {
    const p = this.player;
    if (!p.unit.alive || p.unit.stopTimer > 0 || isRooted(p.unit)) return;
    let mx = 0;
    let my = 0;
    if (this.keys.left.isDown || this.keys.A.isDown) mx -= 1;
    if (this.keys.right.isDown || this.keys.D.isDown) mx += 1;
    if (this.keys.up.isDown || this.keys.W.isDown) my -= 1;
    if (this.keys.down.isDown || this.keys.S.isDown) my += 1;

    const ptr = this.input.activePointer;
    if (mx === 0 && my === 0 && ptr.rightButtonDown() && !this.bar.hit(ptr.x, ptr.y)) {
      const world = this.layers.worldCam.getWorldPoint(ptr.x, ptr.y);
      mx = world.x - p.sprite.x;
      my = world.y - p.sprite.y;
    }

    if (mx === 0 && my === 0) {
      this.tickEnergy(dt, false);
      if (this.keys.E.isDown) this.pushTick();
      return;
    }
    const sprint = this.keys.SHIFT.isDown && !p.unit.energyEmpty && !session.carrying;
    const speed = (sprint ? p.unit.runSpd : p.unit.walkSpd) * speedMul(p.unit) * surviveSpeed(p.unit);
    this.tickEnergy(dt, sprint);
    const len = Math.hypot(mx, my);
    mx /= len;
    my /= len;
    p.unit.facing = angleBetween(0, 0, mx, my);
    this.tryMove(p.sprite, mx * speed * dt, my * speed * dt, p.unit.id);
    this.pushWait = Math.max(0, this.pushWait - dt);
    if (this.keys.E.isDown) this.pushTick();
  }

  private tryMove(sprite: Phaser.GameObjects.Image, dx: number, dy: number, ignoreId?: string): void {
    const nx = sprite.x + dx;
    if (!this.grid.blockedRadius(nx, sprite.y, UNIT_RADIUS, ignoreId)) sprite.x = nx;
    const ny = sprite.y + dy;
    if (!this.grid.blockedRadius(sprite.x, ny, UNIT_RADIUS, ignoreId)) sprite.y = ny;
  }

  private actions(): void {
    if (!this.player.unit.alive) {
      if (this.consume("E") || this.consume("space")) this.revivePlayer();
      return;
    }
    if (this.consume("E")) this.interact();
    if (this.consume("space")) this.cast("melee_player");
    const barKeys = ["ONE", "TWO", "THREE", "FOUR", "FIVE", "SIX", "SEVEN", "EIGHT"];
    barKeys.forEach((name, i) => {
      if (this.consume(name)) this.handleGuiMsg(this.bar.use(i) ?? "");
    });
  }

  private handleGuiMsg(msg: string): void {
    if (!msg) return;
    if (msg.startsWith("cast:")) {
      this.cast(msg.slice(5));
      return;
    }
    if (msg.startsWith("key:")) {
      this.hud.toast("Use E on the matching door");
      return;
    }
    this.hud.toast(msg);
  }

  private interact(): void {
    if (session.carrying) {
      this.putDown();
      return;
    }
    const prop = this.closestProp(28);
    if (!prop) return;
    if (prop.talk) {
      if (!this.talk.start(pickNode(prop.talk))) this.hud.toast("...");
      session.talking = this.talk.open;
      return;
    }
    if (prop.kind === "drop" && prop.loot) {
      this.takeLoot(prop.loot, "Picked up");
      prop.sprite.destroy();
      this.props = this.props.filter((p) => p !== prop);
      return;
    }
    if (prop.kind === "campfire") {
      session.player.warmth = Math.min(100, session.player.warmth + 40);
      this.hud.toast("Heat. Food is still a separate argument.");
      return;
    }
    if (prop.kind === "broken") {
      this.hud.toast("Broken. Repair is the second slot.");
      return;
    }
    if (prop.kind === "toggle") {
      if (prop.used || session.flag(`used:${prop.id}`)) {
        this.hud.toast("Already thrown");
        return;
      }
      prop.used = true;
      session.setFlag(`used:${prop.id}`);
      prop.sprite.setAlpha(0.55);
      runActions(prop.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      return;
    }
    if (prop.kind === "push") {
      if (this.wantsMove()) return;
      this.pickUp(prop);
      return;
    }
    if (prop.kind === "chest" && !prop.used && prop.loot) {
      if (this.lockedAgainst(prop)) {
        this.hud.toast("Locked");
        return;
      }
      if (prop.locked) session.setFlag(`open:${prop.id}`);
      this.takeLoot(prop.loot, "Looted");
      prop.used = true;
      session.setFlag(`used:${prop.id}`);
      prop.sprite.setAlpha(0.45);
      runActions(prop.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      return;
    }
    if (prop.learn) {
      runActions([`learn:${prop.learn}`, ...(prop.actions ?? [])], this.worldCtx()).forEach((n) => this.hud.toast(n));
      prop.used = true;
      session.setFlag(`used:${prop.id}`);
      prop.sprite.setAlpha(0.45);
      return;
    }
    if (prop.kind === "bench") {
      session.guiOpen = true;
      session.window = "inventory";
      this.hud.toast("Crafting bench");
      return;
    }
    if (prop.actions && prop.kind !== "door" && prop.kind !== "hatch") {
      runActions(prop.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      return;
    }
    if (prop.kind === "door" || prop.kind === "hatch") {
      if (prop.requires && !session.flag(prop.requires)) {
        this.hud.toast(prop.lockHint ?? "It will not open yet.");
        return;
      }
      if (this.lockedAgainst(prop)) {
        this.hud.toast("Locked");
        return;
      }
      if (prop.locked) {
        if (!prop.opens) {
          this.hud.toast(prop.lockHint ?? "It will not budge.");
          return;
        }
        inventoryRemove(session.player, keyForLock(prop.opens), 1);
        session.setFlag(`open:${prop.id}`);
        this.setPropLock(prop.id, false);
        if (!prop.toZone) {
          this.hud.toast("Unlocked");
          return;
        }
      }
      if (prop.toZone) this.travel(prop.toZone);
    }
  }

  private lockedAgainst(prop: Prop): boolean {
    if (!prop.locked || !prop.opens) return false;
    if (session.flag(`open:${prop.id}`)) return false;
    return inventoryQuantity(session.player, keyForLock(prop.opens)) <= 0;
  }

  private travel(zone: ZoneId): void {
    this.beginLeave(() => {
      this.dropCarry();
      this.stashZone();
      session.zone = zone;
      session.entry = { x: 0, y: 0 };
      this.scene.restart();
    });
  }

  private cast(spellId: string): void {
    const p = this.player;
    const spell = getSpell(spellId);
    const fx = spellVfx(spellId);
    const target = this.resolveTarget(spell.range, spell.requiresTarget, fx.kind === "melee");
    const dist = target
      ? metres(Phaser.Math.Distance.Between(p.sprite.x, p.sprite.y, target.sprite.x, target.sprite.y))
      : 999;
    const los = target ? hasLos(this.grid, p.sprite.x, p.sprite.y, target.sprite.x, target.sprite.y) : true;
    if (target && spell.reqLos && fx.kind === "bolt" && !los) {
      this.hud.toast("Not in line of sight");
      return;
    }
    if (spellId === "repair") {
      const wreck = this.props.find((pr) => pr.kind === "broken" && this.dist(pr.sprite) < 24);
      if (!wreck) {
        this.hud.toast("Nothing to repair");
        return;
      }
      const repairErr = tryCast(p.unit, spellId, null, dist, true);
      const repairText = spellErrorText(repairErr);
      if (repairText) this.hud.toast(repairText);
      if (repairErr !== "castSuccessful") return;
      wreck.kind = "bench";
      wreck.sprite.setTexture("crate");
      session.setFlag(`used:${wreck.id}`);
      if (wreck.id === "abandoned_car") session.setFlag("cart_repaired");
      if (wreck.actions?.length) runActions(wreck.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      else this.takeLoot([{ id: "wood", qty: 2 }], "Repaired.");
      return;
    }
    if (spellId === "grow") {
      const rad = Phaser.Math.DegToRad(p.unit.facing);
      const tx = p.sprite.x + Math.cos(rad) * PATH_CELL;
      const ty = p.sprite.y + Math.sin(rad) * PATH_CELL;
      const cell = this.grid.worldToCell(tx, ty);
      const wet: { cx: number; cy: number }[] = [];
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const cx = cell.cx + dx;
          const cy = cell.cy + dy;
          if (this.grid.get(cx, cy) === "water") wet.push({ cx, cy });
        }
      }
      if (!wet.length) {
        this.hud.toast("Nothing to grow");
        return;
      }
      const growErr = tryCast(p.unit, spellId, null, 0, true);
      const growText = spellErrorText(growErr);
      if (growText) this.hud.toast(growText);
      if (growErr !== "castSuccessful") return;
      for (const tile of wet) this.grid.set(tile.cx, tile.cy, "grass");
      this.redrawRect(cell.cx - 1, cell.cy - 1, 3, 3);
      this.hud.toast("Green where it was wet.");
      return;
    }
    const err = tryCast(p.unit, spellId, target?.unit ?? null, dist, los);
    const text = spellErrorText(err);
    if (text) this.hud.toast(text);
    if (err !== "castSuccessful") return;
    if (target) {
      p.unit.currentTargetId = target.unit.id;
      p.unit.facing = angleBetween(p.sprite.x, p.sprite.y, target.sprite.x, target.sprite.y);
    }

    if (fx.kind === "melee") this.meleeSwing(spell.range, fx);
    else if (fx.kind === "bolt") this.spawnBolt(spellId, fx, target);
    else {
      this.burst(p.sprite.x, p.sprite.y, "spark");
      if (spell.school === "heal") {
        const hit = schoolDamage(p.unit.spirit, "heal", Boolean(p.unit.flags.crit));
        p.unit.enqueueDamage(-Math.abs(hit.amount), p.unit.id, "heal");
      }
      if (spell.onHit) applyStatus(p.unit, spell.onHit);
    }
  }

  private resolveTarget(range: number, required: boolean, melee: boolean): Actor | null {
    const reach = (range > 0 ? range : melee ? 1.2 : 0) + 0.4;
    const locked = this.currentTarget();
    if (locked) {
      const d = metres(this.dist(locked.sprite));
      if (d <= reach) return locked;
      if (required) return locked;
    }
    return this.nearestEnemy(reach);
  }

  private meleeSwing(range: number, fx: SpellVfx): void {
    const p = this.player;
    const rad = Phaser.Math.DegToRad(p.unit.facing);
    const slash = this.worldImage(p.sprite.x + Math.cos(rad) * 8, p.sprite.y + Math.sin(rad) * 8, "slash");
    slash.setRotation(rad).setDepth(p.sprite.y + 4);
    this.tweens.add({ targets: slash, alpha: 0, duration: 140, onComplete: () => slash.destroy() });

    const locked = this.currentTarget();
    for (const a of this.actors) {
      if (!a.unit.alive || this.asleep.has(a.unit.id)) continue;
      const d = metres(this.dist(a.sprite));
      if (d > range + 0.35) continue;
      const ang = angleBetween(p.sprite.x, p.sprite.y, a.sprite.x, a.sprite.y);
      const inCone = angleDelta(p.unit.facing, ang) <= fx.cone;
      if (inCone || a === locked) {
        this.strike(a, "physical", meleeDamage(p.unit.strength, Boolean(p.unit.flags.crit)), p.unit, "melee_player");
        p.unit.regenEnergy(MELEE_ENERGY_RESTORE);
      }
    }
  }

  private spawnBolt(spellId: string, fx: SpellVfx, target: Actor | null): void {
    const p = this.player.sprite;
    const rad = Phaser.Math.DegToRad(this.player.unit.facing);
    const dest = target
      ? { x: target.sprite.x, y: target.sprite.y }
      : { x: p.x + Math.cos(rad) * 96, y: p.y + Math.sin(rad) * 96 };
    this.launchBolt(this.player, spellId, fx, dest.x, dest.y);
  }

  private launchBolt(caster: Actor, spellId: string, fx: SpellVfx, tx: number, ty: number): void {
    const dx = tx - caster.sprite.x;
    const dy = ty - caster.sprite.y;
    const len = Math.hypot(dx, dy) || 1;
    const sprite = this.worldImage(caster.sprite.x, caster.sprite.y, fx.texture).setDepth(200).setScale(1.4);
    this.bolts.push({
      sprite,
      x: caster.sprite.x,
      y: caster.sprite.y,
      vx: (dx / len) * fx.speed,
      vy: (dy / len) * fx.speed,
      life: 1.4,
      school: fx.school,
      casterId: caster.unit.id,
      radius: fx.radius,
      spellId,
    });
  }

  private tickBolts(dt: number): void {
    const keep: Bolt[] = [];
    for (const bolt of this.bolts) {
      bolt.x += bolt.vx * dt;
      bolt.y += bolt.vy * dt;
      bolt.life -= dt;
      bolt.sprite.setPosition(bolt.x, bolt.y).setDepth(bolt.y + 6);
      if (this.grid.blockedProjectile(bolt.x, bolt.y) || bolt.life <= 0) {
        bolt.sprite.destroy();
        continue;
      }
      const victims = bolt.casterId === "player" ? this.actors : [this.player];
      const hit = victims.find(
        (a) =>
          a.unit.alive &&
          !this.asleep.has(a.unit.id) &&
          Phaser.Math.Distance.Between(bolt.x, bolt.y, a.sprite.x, a.sprite.y) < bolt.radius + 6,
      );
      if (hit) {
        const caster = bolt.casterId === "player" ? this.player.unit : this.actors.find((a) => a.unit.id === bolt.casterId)?.unit;
        const spirit = caster?.spirit ?? session.player.spirit;
        const crit = Boolean(caster?.flags.crit);
        this.strike(hit, bolt.school, schoolDamage(spirit, bolt.school, crit), caster ?? session.player, bolt.spellId);
        bolt.sprite.destroy();
        continue;
      }
      keep.push(bolt);
    }
    this.bolts = keep;
  }

  private burst(x: number, y: number, key: string): void {
    const img = this.worldImage(x, y, key).setDepth(y + 8);
    this.tweens.add({ targets: img, alpha: 0, scale: 1.8, duration: 220, onComplete: () => img.destroy() });
  }

  private strike(
    target: Actor,
    type: DamageType,
    hit: { amount: number; crit: boolean },
    from: Unit = this.player.unit,
    spellId?: string,
  ): void {
    const amount = hit.amount;
    target.unit.enqueueDamage(amount, from.id, type);
    if (from.flags.lifesteal) from.enqueueDamage(-Math.ceil(amount * 0.2), from.id, "heal");
    if (type === "fire" && from.flags.firelash) target.unit.enqueueDamage(Math.ceil(amount * 0.25), from.id, "fire");
    if (type === "frost" && from.flags.winterbite) applyStatus(target.unit, "slow");
    if (from.flags.stranglethorn) applyStatus(target.unit, "poison");
    if (from.flags.sparktongue && target.unit.flags.wet) {
      target.unit.enqueueDamage(Math.ceil(amount * 0.2), from.id, "nature");
    }
    const onHit = spellId ? getSpell(spellId).onHit : undefined;
    if (onHit) applyStatus(target.unit, onHit);
    this.hud.float(target.sprite.x, target.sprite.y - 10, String(amount), hit.crit);
  }

  private ai(dt: number): void {
    const target = {
      unit: this.player.unit,
      x: this.player.sprite.x,
      y: this.player.sprite.y,
    };
    for (const actor of this.actors) {
      if (!actor.unit.alive || this.asleep.has(actor.unit.id)) continue;
      const brain: AiActor = {
        unit: actor.unit,
        x: actor.sprite.x,
        y: actor.sprite.y,
        leashX: actor.leashX,
        leashY: actor.leashY,
        path: actor.path,
        patrol: actor.patrol,
        patrolI: actor.patrolI,
      };
      const events = tickAi(brain, this.player.unit.alive ? target : null, this.grid, dt);
      actor.path = brain.path;
      actor.patrolI = brain.patrolI ?? 0;
      for (const ev of events) {
        if (ev.type === "move") {
          actor.unit.facing = angleBetween(actor.sprite.x, actor.sprite.y, ev.x, ev.y);
          this.tryMove(actor.sprite, ev.x - actor.sprite.x, ev.y - actor.sprite.y, actor.unit.id);
        }
        if (ev.type === "cast") this.enemyCast(actor, ev.spellId);
      }
    }
  }

  private flush(actor: Actor): void {
    const hits = actor.unit.flushIncoming();
    if (hits.length === 0) return;
    if (actor.unit.id === "player" && hits.some((h) => h.type !== "heal")) {
      this.layers.worldCam.shake(80, 0.002);
    }
    if (hits.some((h) => h.type !== "heal")) {
      actor.sprite.setTint(0xffffff);
      this.time.delayedCall(80, () => {
        if (actor.sprite.active) actor.sprite.clearTint();
      });
    }
    if (session.god && actor.unit.id === "player") {
      actor.unit.hp = actor.unit.maxhp;
      actor.unit.alive = true;
      if (actor.unit.creatureState === "dead") actor.unit.creatureState = "idle";
    }
    if (!actor.unit.alive && actor.unit.id !== "player" && !actor.unit.dropped) {
      actor.unit.dropped = true;
      this.spawnDeathLoot(actor);
      session.quests.slay(actor.unit.id, actor.kind);
      if (actor.unit.questUnit) session.dead.add(actor.unit.id);
      const death = getEnemy(actor.kind).onDeath;
      if (death) runActions(death, this.worldCtx()).forEach((n) => this.hud.toast(n));
      this.checkClears();
    }
  }

  private pushTick(): void {
    if (session.carrying || this.pushWait > 0) return;
    const crate = this.props.find((p) => p.kind === "push" && !this.asleep.has(p.id) && this.dist(p.sprite) < 20);
    if (!crate) return;
    const rad = Phaser.Math.DegToRad(this.player.unit.facing);
    const dx = Math.cos(rad) * PATH_CELL;
    const dy = Math.sin(rad) * PATH_CELL;
    const from = this.grid.worldToCell(crate.sprite.x, crate.sprite.y);
    const to = this.grid.worldToCell(crate.sprite.x + dx, crate.sprite.y + dy);
    this.grid.setExtra(from.cx, from.cy, false);
    if (this.grid.solid(to.cx, to.cy)) {
      this.grid.setExtra(from.cx, from.cy, true);
      return;
    }
    const at = Grid.cellWorld(to.cx, to.cy);
    crate.sprite.x = at.x;
    crate.sprite.y = at.y;
    this.grid.setExtra(to.cx, to.cy, true);
    this.pushWait = 0.14;
  }

  private nearestEnemy(maxM: number): Actor | null {
    let best: Actor | null = null;
    let bestD = maxM;
    for (const a of this.actors) {
      if (!a.unit.alive || this.asleep.has(a.unit.id)) continue;
      const d = metres(this.dist(a.sprite));
      if (d < bestD) {
        best = a;
        bestD = d;
      }
    }
    return best;
  }

  private closestProp(px: number): Prop | null {
    let best: Prop | null = null;
    let bestD = px;
    for (const p of this.props) {
      if (!this.canUse(p)) continue;
      const d = this.dist(p.sprite);
      if (d < bestD) {
        best = p;
        bestD = d;
      }
    }
    return best;
  }

  private canUse(p: Prop): boolean {
    if (this.asleep.has(p.id)) return false;
    if (p.kind === "torch" || p.kind === "trigger" || p.kind === "orb") return false;
    if (p.kind === "chest" && (p.used || !p.loot)) return false;
    if (p.kind === "toggle" && (p.used || session.flag(`used:${p.id}`))) return false;
    if (p.kind === "drop" && !p.loot) return false;
    if (p.learn && p.used) return false;
    return true;
  }

  private takeLoot(drops: { id: string; qty: number }[], ok: string): void {
    let full = false;
    for (const drop of drops) {
      if (inventoryAdd(session.player, drop.id, drop.qty) > 0) full = true;
    }
    session.quests.syncAcquire(session.player);
    this.hud.toast(full ? `${ok} Bag is full.` : ok);
  }

  private nearKind(kind: string, px: number): boolean {
    return this.props.some((p) => p.kind === kind && this.dist(p.sprite) < px);
  }

  private dist(obj: Phaser.GameObjects.Image): number {
    return Phaser.Math.Distance.Between(this.player.sprite.x, this.player.sprite.y, obj.x, obj.y);
  }

  private playStart(): { x: number; y: number } {
    const mark = session.marks[session.zone];
    if (
      session.zone === "county" &&
      mark &&
      mark.x < 800 &&
      mark.y < 600 &&
      this.blue.spawn.x > 4000
    ) {
      return this.blue.spawn;
    }
    return this.validStart(mark) ?? this.blue.spawn;
  }

  private validStart(pos: { x: number; y: number } | null | undefined): { x: number; y: number } | null {
    if (!pos || !Number.isFinite(pos.x) || !Number.isFinite(pos.y)) return null;
    if (pos.x <= 0 && pos.y <= 0) return null;
    return pos;
  }

  private noteZone(): void {
    this.hud.toast(ZONE_TITLE[session.zone]);
  }

  private autoAttack(): void {
    const t = this.currentTarget();
    if (!t || !this.player.unit.alive) return;
    if (metres(this.dist(t.sprite)) > 1.2) return;
    if ((this.player.unit.cooldowns.melee_player ?? 0) > 0) return;
    this.cast("melee_player");
  }

  private enemyCast(actor: Actor, spellId: string): void {
    const fx = spellVfx(spellId);
    actor.unit.facing = angleBetween(actor.sprite.x, actor.sprite.y, this.player.sprite.x, this.player.sprite.y);
    if (fx.kind === "melee") {
      this.strike(this.player, "physical", meleeDamage(actor.unit.strength), actor.unit, spellId);
      return;
    }
    if (fx.kind === "bolt") {
      this.launchBolt(actor, spellId, fx, this.player.sprite.x, this.player.sprite.y);
    }
  }

  private spawnDeathLoot(actor: Actor): void {
    const drops = rollLoot(getEnemy(actor.kind).loot);
    for (const drop of drops) {
      const sprite = this.worldImage(actor.sprite.x + 4, actor.sprite.y + 4, "drop");
      this.tweens.add({
        targets: sprite,
        y: sprite.y - 3,
        duration: 680,
        yoyo: true,
        repeat: -1,
        ease: "Sine.easeInOut",
      });
      this.props.push({
        id: `drop:${actor.unit.id}:${drop.id}`,
        kind: "drop",
        x: sprite.x,
        y: sprite.y,
        texture: "drop",
        loot: [drop],
        sprite,
      });
    }
  }

  private pickUp(prop: Prop): void {
    const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
    this.grid.setExtra(cell.cx, cell.cy, false);
    session.carrying = prop.id;
    this.hud.toast("Carrying");
  }

  private putDown(): void {
    const crate = this.props.find((p) => p.id === session.carrying);
    if (!crate) {
      session.carrying = null;
      return;
    }
    const rad = Phaser.Math.DegToRad(this.player.unit.facing);
    const x = this.player.sprite.x + Math.cos(rad) * PATH_CELL;
    const y = this.player.sprite.y + Math.sin(rad) * PATH_CELL;
    const to = this.grid.worldToCell(x, y);
    if (this.grid.solid(to.cx, to.cy, this.player.unit.id)) {
      this.hud.toast("No room");
      return;
    }
    const at = Grid.cellWorld(to.cx, to.cy);
    crate.sprite.x = at.x;
    crate.sprite.y = at.y;
    this.grid.setExtra(to.cx, to.cy, true);
    session.carrying = null;
    this.hud.toast("Set down");
  }

  private followCarry(): void {
    if (!session.carrying) return;
    const crate = this.props.find((p) => p.id === session.carrying);
    if (!crate) {
      session.carrying = null;
      return;
    }
    const rad = Phaser.Math.DegToRad(this.player.unit.facing);
    crate.sprite.x = this.player.sprite.x + Math.cos(rad) * 8;
    crate.sprite.y = this.player.sprite.y + Math.sin(rad) * 8;
    crate.sprite.setDepth(crate.sprite.y + 2);
  }

  private respawnTick(): void {
    for (const actor of this.actors) {
      if (this.asleep.has(actor.unit.id)) continue;
      if (actor.unit.alive || actor.unit.questUnit || actor.unit.respawn <= 0) continue;
      if (actor.unit.deadFor < actor.unit.respawn) continue;
      actor.unit.revive();
      actor.unit.dropped = false;
      const at = this.walkableNear(actor.leashX, actor.leashY, actor.unit.id);
      actor.sprite.setPosition(at.x, at.y).setVisible(true);
    }
  }

  private tickEnergy(dt: number, sprint: boolean): void {
    const p = this.player.unit;
    if (session.carrying) {
      p.spendEnergy(CARRY_ENERGY_DRAIN * dt);
      if (p.energyEmpty) {
        this.forceDrop();
        this.hud.toast("Too tired to carry");
      }
      return;
    }
    if (sprint) p.spendEnergy(30 * dt);
    else p.regenEnergy(30 * dt);
  }

  private worldCtx(): ActionWorld {
    return {
      lock: (id) => {
        if (id === "boss_gate" && session.flag("burial_unlocked")) return;
        this.setPropLock(id, true);
      },
      unlock: (id) => this.setPropLock(id, false),
      setSolid: (id, on) => this.setPropSolid(id, on),
      spawn: (id) => this.spawnFromBlue(id),
      light: (id) => this.lightProp(id),
      aggro: (id) => this.aggroActor(id),
      drain: (id) => this.drainFlood(id),
    };
  }

  private tickTriggers(): void {
    const px = this.player.sprite.x;
    const py = this.player.sprite.y;
    for (const trigger of this.blue.triggers) {
      if (trigger.when && trigger.when !== "enter") continue;
      const inside = inTriggerRect(px, py, trigger.rect);
      if (!inside) {
        this.insideTriggers.delete(trigger.id);
        continue;
      }
      if (this.insideTriggers.has(trigger.id)) continue;
      this.insideTriggers.add(trigger.id);
      if (trigger.once && session.flag(`trig:${trigger.id}`)) continue;
      runActions(trigger.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      if (trigger.once) session.setFlag(`trig:${trigger.id}`);
    }
  }

  private dropCarry(): void {
    this.forceDrop();
  }

  private forceDrop(): void {
    const id = session.carrying;
    if (!id) return;
    const crate = this.props.find((p) => p.id === id);
    session.carrying = null;
    if (!crate) return;
    const at = this.walkableNear(this.player.sprite.x, this.player.sprite.y, this.player.unit.id);
    crate.sprite.setPosition(at.x, at.y);
    const cell = this.grid.worldToCell(at.x, at.y);
    this.grid.setExtra(cell.cx, cell.cy, true);
  }

  private wantsMove(): boolean {
    const ptr = this.input.activePointer;
    return (
      this.keys.left.isDown ||
      this.keys.right.isDown ||
      this.keys.up.isDown ||
      this.keys.down.isDown ||
      this.keys.A.isDown ||
      this.keys.D.isDown ||
      this.keys.W.isDown ||
      this.keys.S.isDown ||
      (ptr.rightButtonDown() && !this.bar.hit(ptr.x, ptr.y))
    );
  }

  private revivePlayer(): void {
    const at = this.walkableNear(this.blue.spawn.x, this.blue.spawn.y, this.player.unit.id);
    this.player.unit.revive();
    this.player.unit.hunger = Math.max(this.player.unit.hunger, 28);
    this.player.unit.warmth = Math.max(this.player.unit.warmth, 40);
    this.player.sprite.setPosition(at.x, at.y);
    this.layers.worldCam.centerOn(at.x, at.y);
    this.lastBlock = null;
    this.syncLoadRing();
    this.hud.toast("You stood up. The county did not wait.");
  }

  private walkableNear(x: number, y: number, ignoreId: string): { x: number; y: number } {
    if (!this.grid.blockedRadius(x, y, UNIT_RADIUS, ignoreId)) return { x, y };
    for (const [dx, dy] of [
      [PATH_CELL, 0],
      [-PATH_CELL, 0],
      [0, PATH_CELL],
      [0, -PATH_CELL],
      [PATH_CELL, PATH_CELL],
      [-PATH_CELL, PATH_CELL],
      [PATH_CELL, -PATH_CELL],
      [-PATH_CELL, -PATH_CELL],
    ]) {
      const nx = x + dx;
      const ny = y + dy;
      if (!this.grid.blockedRadius(nx, ny, UNIT_RADIUS, ignoreId)) return { x: nx, y: ny };
    }
    return { x, y };
  }

  private stashZone(): void {
    if (!this.player) return;
    this.wakeAll();
    this.captureZone();
    session.markHere(this.player.sprite.x, this.player.sprite.y);
  }

  private writeSlot(): void {
    this.stashZone();
    session.save();
    this.lastBlock = null;
    this.syncLoadRing();
    this.hud.toast("Saved slot 0");
  }

  private census(): WorldInspect {
    return {
      zone: session.zone,
      player: { x: this.player.sprite.x, y: this.player.sprite.y },
      actors: this.actors.map((a) => ({
        id: a.unit.id,
        kind: a.kind,
        x: a.sprite.x,
        y: a.sprite.y,
        hp: a.unit.hp,
        maxhp: a.unit.maxhp,
        asleep: this.asleep.has(a.unit.id),
        alive: a.unit.alive,
      })),
      props: this.props.map((p) => ({
        id: p.id,
        kind: p.kind,
        x: p.sprite.x,
        y: p.sprite.y,
      })),
      bolts: this.bolts.length,
      chunks: this.tileChunks.size,
      asleep: this.asleep.size,
    };
  }

  private captureZone(): void {
    session.writeZone(session.zone, {
      actors: this.actors.map((a) => ({
        id: a.unit.id,
        hp: a.unit.hp,
        mp: a.unit.mp,
        x: a.sprite.x,
        y: a.sprite.y,
        alive: a.unit.alive,
        facing: a.unit.facing,
        deadFor: a.unit.deadFor,
      })),
      props: this.props
        .filter((p) => p.kind !== "drop")
        .map((p) => ({
          id: p.id,
          used: p.used,
          locked: p.locked,
          x: p.sprite.x,
          y: p.sprite.y,
        })),
      fog: this.gui.fog.pack(),
    });
  }

  private applyZoneSnap(): void {
    const snap = session.readZone(session.zone);
    if (!snap) return;
    for (const row of snap.actors) {
      const actor = this.actors.find((a) => a.unit.id === row.id);
      if (!actor) continue;
      actor.unit.hp = row.hp;
      if (row.mp != null) actor.unit.mp = row.mp;
      actor.unit.alive = row.alive;
      if (row.facing != null) actor.unit.facing = row.facing;
      if (row.deadFor != null) actor.unit.deadFor = row.deadFor;
      actor.sprite.setPosition(row.x, row.y);
      if (!row.alive) actor.sprite.setVisible(false);
    }
    for (const row of snap.props) {
      const prop = this.props.find((p) => p.id === row.id);
      if (!prop) continue;
      prop.used = row.used ?? prop.used;
      prop.locked = row.locked ?? prop.locked;
      prop.x = row.x;
      prop.y = row.y;
      prop.sprite.setPosition(row.x, row.y);
      if (prop.used) prop.sprite.setAlpha(0.45);
    }
  }

  private applyPropSolids(): void {
    for (const prop of this.props) {
      if (prop.kind === "push" && prop.id !== session.carrying) {
        const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
        this.grid.setExtra(cell.cx, cell.cy, true);
      }
      if (prop.locked && !prop.toZone) {
        const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
        this.grid.setExtra(cell.cx, cell.cy, true);
      }
    }
  }

  private applyDrainedFloods(): void {
    for (const flood of this.blue.floods ?? []) {
      if (!session.flag(`drain:${flood.id}`)) continue;
      this.grid.fillRect(flood.x, flood.y, flood.w, flood.h, "stone");
    }
  }

  private drainFlood(id: string): void {
    const flood = this.blue.floods?.find((f) => f.id === id);
    if (!flood) return;
    this.grid.fillRect(flood.x, flood.y, flood.w, flood.h, "stone");
    session.setFlag(`drain:${id}`);
    this.redrawRect(flood.x, flood.y, flood.w, flood.h);
  }

  private redrawRect(cx: number, cy: number, w: number, h: number): void {
    const c0 = Math.floor(cx / TILE_CHUNK_CELLS);
    const r0 = Math.floor(cy / TILE_CHUNK_CELLS);
    const c1 = Math.floor((cx + w - 1) / TILE_CHUNK_CELLS);
    const r1 = Math.floor((cy + h - 1) / TILE_CHUNK_CELLS);
    for (let y = r0; y <= r1; y++) {
      for (let x = c0; x <= c1; x++) {
        const key = `${x},${y}`;
        this.tileChunks.get(key)?.destroy();
        this.tileChunks.delete(key);
      }
    }
    this.lastBlock = null;
    this.syncLoadRing();
  }

  private checkClears(): void {
    for (const trigger of this.blue.triggers) {
      if (trigger.when !== "clear") continue;
      if (trigger.once && session.flag(`trig:${trigger.id}`)) continue;
      const ids = trigger.watch ?? [];
      if (ids.length === 0) continue;
      const done = ids.every((id) => {
        const actor = this.actors.find((a) => a.unit.id === id);
        return !actor || !actor.unit.alive;
      });
      if (!done) continue;
      runActions(trigger.actions, this.worldCtx()).forEach((n) => this.hud.toast(n));
      if (trigger.once) session.setFlag(`trig:${trigger.id}`);
    }
  }

  private setPropLock(id: string, on: boolean): void {
    const prop = this.props.find((p) => p.id === id);
    if (!prop) return;
    prop.locked = on;
    const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
    this.grid.setExtra(cell.cx, cell.cy, on);
    prop.sprite.setAlpha(on ? 1 : 0.35);
  }

  private setPropSolid(id: string, on: boolean): void {
    const prop = this.props.find((p) => p.id === id);
    if (!prop) return;
    const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
    this.grid.setExtra(cell.cx, cell.cy, on);
  }

  private spawnFromBlue(id: string): void {
    if (this.actors.some((a) => a.unit.id === id) || session.dead.has(id)) return;
    const spec = this.blue.enemies.find((e) => e.id === id);
    if (!spec) return;
    const actor = this.spawnEnemy(spec);
    actor.patrol = spec.patrol;
    this.actors.push(actor);
  }

  private lightProp(id: string): void {
    const prop = this.props.find((p) => p.id === id);
    if (!prop) return;
    if (this.extraLights.some((l) => l.x === prop.sprite.x && l.y === prop.sprite.y)) return;
    this.extraLights.push({ x: prop.sprite.x, y: prop.sprite.y, radius: 32 });
  }

  private aggroActor(id: string): void {
    const actor = this.actors.find((a) => a.unit.id === id);
    if (!actor?.unit.alive) return;
    actor.unit.combatState = "combat";
    actor.unit.currentTargetId = this.player.unit.id;
    this.wakeId(actor.unit.id);
  }

  private wakeAll(): void {
    this.asleep.clear();
    for (const actor of this.actors) {
      actor.sprite.setVisible(actor.unit.alive);
    }
    for (const prop of this.props) {
      prop.sprite.setVisible(true);
    }
    this.lastBlock = null;
  }

  private wakeId(id: string): void {
    this.asleep.delete(id);
    const actor = this.actors.find((a) => a.unit.id === id);
    if (actor) actor.sprite.setVisible(actor.unit.alive);
    const prop = this.props.find((p) => p.id === id);
    if (prop) prop.sprite.setVisible(true);
  }

  private syncLoadRing(): void {
    const px = this.player.sprite.x;
    const py = this.player.sprite.y;
    const block = worldBlock(px, py);
    if (sameBlock(this.lastBlock, block)) return;
    this.lastBlock = block;

    for (const actor of this.actors) {
      const stay =
        actor.unit.combatState !== "idle" ||
        inLoadRing(actor.sprite.x, actor.sprite.y, px, py);
      this.setAsleep(actor.unit.id, !stay, actor.sprite, actor.unit.alive);
    }

    for (const prop of this.props) {
      const stay =
        prop.id === session.carrying ||
        Boolean(prop.locked && !prop.toZone) ||
        Boolean(session.talking && prop.talk) ||
        inLoadRing(prop.sprite.x, prop.sprite.y, px, py);
      this.setAsleep(prop.id, !stay, prop.sprite, true);
      if (stay && prop.kind === "push" && prop.id !== session.carrying) {
        const cell = this.grid.worldToCell(prop.sprite.x, prop.sprite.y);
        this.grid.setExtra(cell.cx, cell.cy, true);
      }
    }

    const chunkCols = Math.ceil(this.grid.cols / TILE_CHUNK_CELLS);
    const chunkRows = Math.ceil(this.grid.rows / TILE_CHUNK_CELLS);
    const keep = new Set<string>();
    for (let cy = 0; cy < chunkRows; cy++) {
      for (let cx = 0; cx < chunkCols; cx++) {
        const key = `${cx},${cy}`;
        if (!chunkInRing(cx, cy, px, py)) continue;
        keep.add(key);
        if (!this.tileChunks.has(key)) this.tileChunks.set(key, this.drawChunk(cx, cy));
        else this.tileChunks.get(key)?.setVisible(true);
      }
    }
    for (const [key, rt] of this.tileChunks) {
      if (keep.has(key)) continue;
      rt.destroy();
      this.tileChunks.delete(key);
    }
  }

  private setAsleep(id: string, sleep: boolean, sprite: Phaser.GameObjects.Image, alive: boolean): void {
    if (sleep) {
      this.asleep.add(id);
      sprite.setVisible(false);
      this.grid.clearOccupy(id);
    } else {
      this.asleep.delete(id);
      sprite.setVisible(alive);
    }
  }

  private drawDebugGrid(): void {
    this.gridGfx?.destroy();
    if (!this.showGrid) return;
    const view = this.layers.worldCam.worldView;
    const pad = 4;
    const x0 = Math.max(0, Math.floor(view.x / PATH_CELL) - pad);
    const y0 = Math.max(0, Math.floor(view.y / PATH_CELL) - pad);
    const x1 = Math.min(this.grid.cols, Math.ceil((view.x + view.width) / PATH_CELL) + pad);
    const y1 = Math.min(this.grid.rows, Math.ceil((view.y + view.height) / PATH_CELL) + pad);
    const g = this.add.graphics().setDepth(50);
    g.lineStyle(1, 0xffffff, 0.18);
    for (let x = x0; x <= x1; x++) g.lineBetween(x * PATH_CELL, y0 * PATH_CELL, x * PATH_CELL, y1 * PATH_CELL);
    for (let y = y0; y <= y1; y++) g.lineBetween(x0 * PATH_CELL, y * PATH_CELL, x1 * PATH_CELL, y * PATH_CELL);
    this.layers.addWorld(g);
    this.gridGfx = g;
  }
}
