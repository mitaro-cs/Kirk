import { mount } from 'svelte';
import './app.css';
import Settings from './settings/Settings.svelte';

mount(Settings, { target: document.getElementById('app')! });
