// Commit-graph lane layout.
//
// Turns a newest-first list of commits (each with parent ids) into per-row
// lane assignments and the line segments that connect them. Lanes route on
// hard diagonals — no beziers — per Plumb's zero-radius rule.
//
// The algorithm is the standard incremental "active lanes" model:
//  - `incoming` holds, per column, the commit id that column is currently
//    waiting to reach (reserved by an already-drawn child).
//  - A commit takes the left-most column reserved for it (or a fresh column).
//  - It then reserves columns for its parents: its first parent reuses the
//    commit's own column; extra parents (merges) take fresh columns. If a
//    parent is already reserved elsewhere, the left-most reservation wins and
//    the lanes converge.

import type { CommitRow } from "./git";

export const LANE_W = 18; // horizontal distance between lanes, px
export const ROW_H = 34; // must match --row-commit
export const NODE_R = 4.5;
const LANES = 7;

export interface Segment {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  lane: number; // palette index (0..6)
}

export interface GraphNode {
  col: number;
  y: number;
  lane: number; // palette index
  merge: boolean;
  head: boolean;
}

export interface GraphLayout {
  nodes: GraphNode[];
  segments: Segment[];
  width: number;
  height: number;
}

const colX = (col: number) => col * LANE_W + LANE_W / 2;
const paletteFor = (col: number) => col % LANES;

/** Left-most free column in `lanes` (first hole, else append index). */
function freeColumn(lanes: (string | undefined)[]): number {
  const hole = lanes.findIndex((v) => v === undefined);
  return hole === -1 ? lanes.length : hole;
}

export function layoutGraph(commits: CommitRow[]): GraphLayout {
  const nodes: GraphNode[] = [];
  const segments: Segment[] = [];
  let incoming: (string | undefined)[] = [];
  let maxCol = 0;

  commits.forEach((commit, row) => {
    const yTop = row * ROW_H;
    const yMid = yTop + ROW_H / 2;
    const yBot = yTop + ROW_H;

    // Which column does this commit sit in?
    let col = incoming.indexOf(commit.id);
    if (col === -1) col = freeColumn(incoming);

    const paletteAtNode = paletteFor(col);

    // Build the outgoing arrangement from a copy of incoming.
    const outgoing = incoming.slice();
    // Clear every column that was waiting on this commit (merges collapse).
    for (let c = 0; c < outgoing.length; c++) {
      if (outgoing[c] === commit.id) outgoing[c] = undefined;
    }

    // Place parents: first parent keeps this column; the rest branch out.
    const parentCols: { col: number; parent: string }[] = [];
    commit.parents.forEach((parent, i) => {
      if (i === 0) {
        outgoing[col] = parent;
        parentCols.push({ col, parent });
      } else {
        // Reuse an existing reservation for this parent if one exists, so
        // shared ancestors converge instead of spawning duplicate lanes.
        let pc = outgoing.indexOf(parent);
        if (pc === -1) pc = freeColumn(outgoing);
        outgoing[pc] = parent;
        parentCols.push({ col: pc, parent });
      }
    });

    // --- Segments for this row band ---

    // 1. Columns that pass straight through, untouched by this commit.
    const touched = new Set<number>([col, ...parentCols.map((p) => p.col)]);
    const span = Math.max(incoming.length, outgoing.length);
    for (let c = 0; c < span; c++) {
      const passes = incoming[c] !== undefined && incoming[c] === outgoing[c];
      if (passes && !touched.has(c)) {
        segments.push({ x1: colX(c), y1: yTop, x2: colX(c), y2: yBot, lane: paletteFor(c) });
      }
    }

    // 2. Incoming edges: any column that was waiting on this commit converges
    //    into the node (top half of the band).
    for (let c = 0; c < incoming.length; c++) {
      if (incoming[c] === commit.id) {
        segments.push({ x1: colX(c), y1: yTop, x2: colX(col), y2: yMid, lane: paletteFor(c) });
      }
    }

    // 3. Outgoing edges: from the node down to each parent's column
    //    (bottom half of the band), coloured by the destination lane.
    for (const { col: pc } of parentCols) {
      segments.push({ x1: colX(col), y1: yMid, x2: colX(pc), y2: yBot, lane: paletteFor(pc) });
    }

    nodes.push({
      col,
      y: yMid,
      lane: paletteAtNode,
      merge: commit.is_merge,
      head: commit.refs.some((r) => r.startsWith("HEAD")),
    });

    maxCol = Math.max(maxCol, span - 1, col, ...parentCols.map((p) => p.col));
    incoming = outgoing;
  });

  return {
    nodes,
    segments,
    width: (maxCol + 1) * LANE_W,
    height: commits.length * ROW_H,
  };
}
