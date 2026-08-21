<script setup lang="ts">
import type { Destination } from '../types'
import DestinationForm from '../components/destinations/DestinationForm.vue'
import DestinationList from '../components/destinations/DestinationList.vue'
defineProps<{ destinations: readonly Destination[] }>()
const emit = defineEmits<{ create: [input: { name: string; url: string }]; toggle: [destination: Destination, enabled: boolean]; test: [id: string]; rotate: [id: string]; finishRotation: [id: string]; remove: [id: string] }>()
function confirmRemove(id: string) { if (confirm('Delete this destination? Event history will be retained.')) emit('remove', id) }
function confirmRotate(id: string) { if (confirm('Rotate the signing secret? Store the new value before updating the receiver.')) emit('rotate', id) }
</script>
<template><section class="split"><DestinationForm @create="emit('create',$event)" /><DestinationList :destinations="destinations" @toggle="(destination,enabled)=>emit('toggle',destination,enabled)" @test="emit('test',$event)" @rotate="confirmRotate" @finish-rotation="emit('finishRotation',$event)" @remove="confirmRemove" /></section></template>
<style scoped>.split{display:grid;grid-template-columns:minmax(280px,.65fr) minmax(0,1.35fr);gap:18px}@media(max-width:860px){.split{grid-template-columns:1fr}}</style>
