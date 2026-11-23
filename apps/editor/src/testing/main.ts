import '../app.css';
import GraphHarness from './GraphHarness.svelte';
import { mount } from 'svelte';
mount(GraphHarness, { target: document.getElementById('app')! });
