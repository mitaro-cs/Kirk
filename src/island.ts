import { mount } from 'svelte';
import './app.css';
import Island from './island/Island.svelte';

mount(Island, { target: document.getElementById('app')! });
