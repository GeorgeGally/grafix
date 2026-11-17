<script lang="ts">
  import { SvelteFlowProvider } from '@xyflow/svelte';
  import GraphEditor from '../lib/GraphEditor.svelte';
</script>
<SvelteFlowProvider>
  <GraphEditor ready={true}/>
</SvelteFlowProvider>
