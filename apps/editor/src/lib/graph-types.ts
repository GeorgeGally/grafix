export type PortType = 'geometry' | 'scene_object' | 'instance_set' | 'texture2_d' | 'light' | 'scene' | 'camera' | 'effect' | 'float';
export interface PortSpec { name: string; data_type: PortType }
export interface ParamSpec { name: string; kind: 'number' | 'enum'; default: number | string; min?: number; max?: number; options?: string[] }
export interface NodeSpec { type_id: string; title: string; category: string; description: string; inputs: PortSpec[]; outputs: PortSpec[]; params: ParamSpec[] }
export interface GraphNode { id: string; type: string; version: number; params: Record<string, number | string>; ui: { x: number; y: number } }
export interface GraphEdge { from: { node: string; port: string }; to: { node: string; port: string } }
export interface GraphDocument { format_version: number; name: string; nodes: GraphNode[]; edges: GraphEdge[] }
export interface GraphCardData { spec: NodeSpec; [key: string]: unknown }
export const PROJECT_STORAGE_KEY = 'grafix:graph:v1';
