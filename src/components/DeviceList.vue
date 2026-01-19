<template>
  <div class="space-y-2">
    <div
      v-for="device in devices"
      :key="device.ip"
      class="flex items-center justify-between p-3 bg-dark-700 rounded-lg hover:bg-dark-600 transition-colors border border-dark-600"
    >
      <div class="flex items-center space-x-3">
        <!-- Device type icon -->
        <div
          class="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0"
          :class="getDeviceIconBgClass(device)"
          :title="device.vendor || 'Unknown device'"
        >
          <component :is="getDeviceIcon(device)" class="w-4 h-4" />
        </div>
        <div class="min-w-0">
          <div class="font-mono text-sm font-medium text-white">{{ device.ip }}</div>
          <div v-if="device.hostname" class="text-xs text-gray-400 truncate">{{ device.hostname }}</div>
          <!-- Vendor display - show vendor if available, otherwise MAC -->
          <div v-if="device.vendor" class="text-xs text-brand-cyan truncate">{{ device.vendor }}</div>
          <div v-else-if="device.mac" class="text-xs text-gray-500 font-mono">{{ device.mac }}</div>
        </div>
      </div>
      <!-- Status indicator and response time -->
      <div class="flex items-center gap-2 flex-shrink-0">
        <div
          class="w-2 h-2 rounded-full"
          :class="getDeviceStatusClass(device)"
          :title="getDeviceStatusTitle(device)"
        ></div>
        <div v-if="device.responseTimeMs !== null && device.responseTimeMs !== undefined" class="text-xs font-mono" :class="device.responseTimeMs > 0 ? 'text-brand-cyan' : 'text-yellow-500'">
          {{ device.responseTimeMs > 0 ? `${device.responseTimeMs.toFixed(1)}ms` : 'ARP' }}
        </div>
      </div>
    </div>
    <div v-if="devices.length === 0" class="text-center text-gray-500 py-8">
      <svg class="w-12 h-12 mx-auto mb-3 text-gray-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
      </svg>
      <p>No devices found</p>
      <p class="text-xs text-gray-600 mt-1">Click "Scan Now" to discover devices on your network</p>
    </div>
  </div>
</template>

<script setup lang="ts">
import { h } from 'vue'
import type { Device } from '@/stores/agent'

defineProps<{
  devices: Device[]
}>()

// SVG Icon Components
const RouterIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M8.288 15.038a5.25 5.25 0 017.424 0M5.106 11.856c3.807-3.808 9.98-3.808 13.788 0M1.924 8.674c5.565-5.565 14.587-5.565 20.152 0M12.53 18.22l-.53.53-.53-.53a.75.75 0 011.06 0z' })
])

const AppleIcon = () => h('svg', { fill: 'currentColor', viewBox: '0 0 24 24' }, [
  h('path', { d: 'M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M13 3.5c.73-.83 1.94-1.46 2.94-1.5.13 1.17-.34 2.35-1.04 3.19-.69.85-1.83 1.51-2.95 1.42-.15-1.15.41-2.35 1.05-3.11z' })
])

const NasIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7m0 0a3 3 0 01-3 3m0 3h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008zm-3 6h.008v.008h-.008v-.008zm0-6h.008v.008h-.008v-.008z' })
])

const IotIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M12 18v-5.25m0 0a6.01 6.01 0 001.5-.189m-1.5.189a6.01 6.01 0 01-1.5-.189m3.75 7.478a12.06 12.06 0 01-4.5 0m3.75 2.383a14.406 14.406 0 01-3 0M14.25 18v-.192c0-.983.658-1.823 1.508-2.316a7.5 7.5 0 10-7.517 0c.85.493 1.509 1.333 1.509 2.316V18' })
])

const PrinterIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M6.72 13.829c-.24.03-.48.062-.72.096m.72-.096a42.415 42.415 0 0110.56 0m-10.56 0L6.34 18m10.94-4.171c.24.03.48.062.72.096m-.72-.096L17.66 18m0 0l.229 2.523a1.125 1.125 0 01-1.12 1.227H7.231c-.662 0-1.18-.568-1.12-1.227L6.34 18m11.318 0h1.091A2.25 2.25 0 0021 15.75V9.456c0-1.081-.768-2.015-1.837-2.175a48.055 48.055 0 00-1.913-.247M6.34 18H5.25A2.25 2.25 0 013 15.75V9.456c0-1.081.768-2.015 1.837-2.175a48.041 48.041 0 011.913-.247m10.5 0a48.536 48.536 0 00-10.5 0m10.5 0V3.375c0-.621-.504-1.125-1.125-1.125h-8.25c-.621 0-1.125.504-1.125 1.125v3.659M18 10.5h.008v.008H18V10.5zm-3 0h.008v.008H15V10.5z' })
])

const GamingIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M5.25 5.653c0-.856.917-1.398 1.667-.986l11.54 6.348a1.125 1.125 0 010 1.971l-11.54 6.347a1.125 1.125 0 01-1.667-.985V5.653z' })
])

const MobileIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M10.5 1.5H8.25A2.25 2.25 0 006 3.75v16.5a2.25 2.25 0 002.25 2.25h7.5A2.25 2.25 0 0018 20.25V3.75a2.25 2.25 0 00-2.25-2.25H13.5m-3 0V3h3V1.5m-3 0h3m-3 18.75h3' })
])

const ComputerIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M9 17.25v1.007a3 3 0 01-.879 2.122L7.5 21h9l-.621-.621A3 3 0 0115 18.257V17.25m6-12V15a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 15V5.25m18 0A2.25 2.25 0 0018.75 3H5.25A2.25 2.25 0 003 5.25m18 0V12a2.25 2.25 0 01-2.25 2.25H5.25A2.25 2.25 0 013 12V5.25' })
])

const UnknownIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M21 12a9 9 0 11-18 0 9 9 0 0118 0zm-9 5.25h.008v.008H12v-.008z' })
])

// Firewall icon (shield)
const FirewallIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z' })
])

// Service/VM icon (cube/container)
const ServiceIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'm21 7.5-9-5.25L3 7.5m18 0-9 5.25m9-5.25v9l-9 5.25M3 7.5l9 5.25M3 7.5v9l9 5.25m0-9v9' })
])

// Server icon (server rack)
const ServerIcon = () => h('svg', { fill: 'none', viewBox: '0 0 24 24', stroke: 'currentColor', 'stroke-width': '1.5' }, [
  h('path', { 'stroke-linecap': 'round', 'stroke-linejoin': 'round', d: 'M21.75 17.25v-.228a4.5 4.5 0 00-.12-1.03l-2.268-9.64a3.375 3.375 0 00-3.285-2.602H7.923a3.375 3.375 0 00-3.285 2.602l-2.268 9.64a4.5 4.5 0 00-.12 1.03v.228m19.5 0a3 3 0 01-3 3H5.25a3 3 0 01-3-3m19.5 0a3 3 0 00-3-3H5.25a3 3 0 00-3 3m16.5 0h.008v.008h-.008v-.008zm-3 0h.008v.008h-.008v-.008z' })
])

// Get the icon component based on device type
function getDeviceIcon(device: Device) {
  const deviceType = device.deviceType || inferDeviceTypeFromVendor(device.vendor)
  
  switch (deviceType) {
    case 'firewall': return FirewallIcon
    case 'router': return RouterIcon
    case 'service': return ServiceIcon
    case 'server': return ServerIcon
    case 'apple': return AppleIcon
    case 'nas': return NasIcon
    case 'iot': return IotIcon
    case 'printer': return PrinterIcon
    case 'gaming': return GamingIcon
    case 'mobile': return MobileIcon
    case 'computer': return ComputerIcon
    default: return UnknownIcon
  }
}

// Get background class for device icon
function getDeviceIconBgClass(device: Device): string {
  const deviceType = device.deviceType || inferDeviceTypeFromVendor(device.vendor)
  const base = 'bg-dark-600'
  
  switch (deviceType) {
    case 'firewall': return `${base} text-rose-400`
    case 'router': return `${base} text-blue-400`
    case 'service': return `${base} text-emerald-400`
    case 'server': return `${base} text-amber-400`
    case 'apple': return `${base} text-gray-300`
    case 'nas': return `${base} text-purple-400`
    case 'iot': return `${base} text-green-400`
    case 'printer': return `${base} text-orange-400`
    case 'gaming': return `${base} text-red-400`
    case 'mobile': return `${base} text-cyan-400`
    case 'computer': return `${base} text-indigo-400`
    default: return `${base} text-gray-400`
  }
}

// Fallback inference from vendor name (in case backend didn't set deviceType)
function inferDeviceTypeFromVendor(vendor?: string): string | undefined {
  if (!vendor) return undefined
  
  const v = vendor.toLowerCase()
  
  // Firewall / Security appliances (check first)
  if (v.includes('firewalla') || v.includes('pfsense') || v.includes('opnsense') ||
      v.includes('sophos') || v.includes('watchguard') || v.includes('sonicwall') ||
      v.includes('barracuda') || v.includes('checkpoint') || v.includes('forcepoint')) {
    return 'firewall'
  }
  
  // Virtualization / Containers / Services
  if (v.includes('proxmox') || v.includes('vmware') || v.includes('xensource') ||
      v.includes('parallels') || v.includes('virtualbox') || v.includes('qemu') ||
      v.includes('docker') || v.includes('kubernetes') || v.includes('virtual machine')) {
    return 'service'
  }
  
  // Network equipment (routers, switches, APs)
  if (v.includes('cisco') || v.includes('ubiquiti') || v.includes('netgear') || 
      v.includes('tp-link') || v.includes('linksys') || v.includes('d-link') ||
      v.includes('mikrotik') || v.includes('aruba') || v.includes('juniper') ||
      v.includes('zyxel') || v.includes('draytek') || v.includes('meraki') ||
      v.includes('routerboard') || v.includes('fortinet') || v.includes('palo alto')) {
    return 'router'
  }
  
  // Server hardware
  if (v.includes('supermicro') || v.includes('dell emc') || v.includes('hpe') ||
      v.includes('hewlett packard enterprise') || v.includes('ibm') || v.includes('oracle') ||
      v.includes('fujitsu') || v.includes('inspur')) {
    return 'server'
  }
  
  if (v.includes('apple')) return 'apple'
  if (v.includes('synology') || v.includes('qnap') || v.includes('western digital') ||
      v.includes('ugreen') || v.includes('asustor') || v.includes('terramaster')) {
    return 'nas'
  }
  if (v.includes('sonos') || v.includes('philips') || v.includes('ring') || 
      v.includes('nest') || v.includes('amazon') || v.includes('google') ||
      v.includes('espressif') || v.includes('tuya') || v.includes('shelly') ||
      v.includes('wemo') || v.includes('lifx') || v.includes('nanoleaf')) {
    return 'iot'
  }
  if (v.includes('hewlett packard') || v.includes('hp inc') || v.includes('canon') || 
      v.includes('epson') || v.includes('brother') || v.includes('xerox') ||
      v.includes('lexmark') || v.includes('ricoh') || v.includes('konica') ||
      v.includes('kyocera')) {
    return 'printer'
  }
  if (v.includes('sony') || v.includes('nintendo') || v.includes('microsoft') || v.includes('valve')) {
    return 'gaming'
  }
  if (v.includes('samsung') || v.includes('huawei') || v.includes('xiaomi') || 
      v.includes('oneplus') || v.includes('oppo') || v.includes('vivo') ||
      v.includes('motorola') || v.includes('lg electronics') || v.includes('realme')) {
    return 'mobile'
  }
  if (v.includes('dell') || v.includes('lenovo') || v.includes('acer') || 
      v.includes('asus') || v.includes('intel') || v.includes('realtek') ||
      v.includes('gigabyte') || v.includes('msi') || v.includes('toshiba')) {
    return 'computer'
  }
  
  return undefined
}

// Determine device status based on response time
function getDeviceStatusClass(device: Device): string {
  if (device.responseTimeMs === null || device.responseTimeMs === undefined) {
    return 'bg-gray-500' // Unknown status
  }
  // Both ping responders and ARP-detected devices show as green (online)
  return 'bg-green-500'
}

function getDeviceStatusTitle(device: Device): string {
  if (device.responseTimeMs === null || device.responseTimeMs === undefined) {
    return 'Status unknown'
  }
  if (device.responseTimeMs > 0) {
    return 'Online - responding to ping'
  }
  return 'Online - detected via ARP'
}
</script>
