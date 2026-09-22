// Group branch names into a folder tree by their "/" segments
// (feature/deploy, feature/styling → a "feature" folder with two leaves).
import type { BranchInfo } from "./git";

export interface BranchNode {
  name: string; // segment label
  path: string; // full path to this node
  branch?: BranchInfo; // set on leaves
  children: BranchNode[];
}

export function buildBranchTree(branches: BranchInfo[]): BranchNode[] {
  const root: BranchNode = { name: "", path: "", children: [] };
  for (const b of branches) {
    const parts = b.name.split("/");
    let cur = root;
    parts.forEach((seg, i) => {
      const isLeaf = i === parts.length - 1;
      const path = parts.slice(0, i + 1).join("/");
      if (isLeaf) {
        cur.children.push({ name: seg, path, branch: b, children: [] });
      } else {
        let folder = cur.children.find((c) => c.name === seg && !c.branch);
        if (!folder) {
          folder = { name: seg, path, children: [] };
          cur.children.push(folder);
        }
        cur = folder;
      }
    });
  }
  sort(root);
  return root.children;
}

function sort(node: BranchNode) {
  node.children.sort((a, b) => {
    const af = a.branch ? 1 : 0; // folders (0) before leaves (1)
    const bf = b.branch ? 1 : 0;
    if (af !== bf) return af - bf;
    return a.name.localeCompare(b.name);
  });
  node.children.forEach(sort);
}

/** Leaves in display order (folders first, then names) — the order a shift-click range spans. */
export function flattenBranches(nodes: BranchNode[]): BranchInfo[] {
  const out: BranchInfo[] = [];
  const walk = (ns: BranchNode[]) => {
    for (const n of ns) {
      if (n.branch) out.push(n.branch);
      else walk(n.children);
    }
  };
  walk(nodes);
  return out;
}
