export type PortType = 'geometry' | 'effector' | 'material' | 'scene_object' | 'instance_set' | 'texture2_d' | 'light' | 'scene' | 'camera' | 'effect' | 'float';
export interface PortSpec { name: string; data_type: PortType }
export type GraphParamValue = number | string | boolean | number[];
export interface ParamSpec {
  name: string;
  kind: 'number' | 'enum' | 'bool' | 'color' | 'asset_ref';
  default: GraphParamValue;
  min?: number;
  max?: number;
  options?: string[];
  group?: string;
  unit?: string;
}
export interface NodeSpec { type_id: string; title: string; category: string; description: string; inputs: PortSpec[]; outputs: PortSpec[]; params: ParamSpec[] }
export interface GraphNode { id: string; type: string; label?: string; version: number; params: Record<string, GraphParamValue>; ui: { x: number; y: number } }
export interface GraphEdge { from: { node: string; port: string }; to: { node: string; port: string } }
export interface AnimationKey { time: number; value: number }
export interface AnimationTrack {
  node: string;
  parameter: string;
  interpolation: 'linear' | 'step';
  keys: AnimationKey[];
}
export interface GraphDocument {
  format_version: number;
  name: string;
  nodes: GraphNode[];
  edges: GraphEdge[];
  tracks?: AnimationTrack[];
}
export interface NodeExecutionStatus {
  id: string;
  state: 'active' | 'inactive' | 'partial' | 'incomplete' | 'unsupported';
  message: string;
}
export interface GraphCardData {
  spec: NodeSpec;
  label?: string;
  execution?: NodeExecutionStatus;
  [key: string]: unknown;
}
export const PROJECT_STORAGE_KEY = 'grafix:graph:v1';

/** Selection mirrored into the viewport for axis drag controls. */
export interface SpatialSelection {
  id: string;
  title: string;
  active: boolean;
  position: { x: number; y: number; z: number };
}
