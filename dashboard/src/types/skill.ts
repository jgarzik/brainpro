/**
 * Skill pack types
 */

/** Skill frontmatter metadata */
export interface SkillFrontmatter {
  name: string;
  description: string;
  version?: string;
  author?: string;
  tags?: string[];
  requirements?: string[];
}

/** Skill pack */
export interface SkillPack {
  id: string;
  name: string;
  description: string;
  source: "builtin" | "project" | "user";
  active: boolean;
  frontmatter: SkillFrontmatter;
  content?: string;
}

/** Skill activation status */
export interface SkillStatus {
  id: string;
  active: boolean;
  last_used?: number;
  use_count: number;
}
