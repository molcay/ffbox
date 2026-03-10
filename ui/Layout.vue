<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'
import { globalState } from './store'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

const route = useRoute()
const router = useRouter()

let unlistenProgress: (() => void) | undefined

async function checkDependencies() {
  globalState.isCheckingDependencies = true
  try {
    const [settings] = await Promise.all([
      invoke('get_settings'),
      new Promise(resolve => setTimeout(resolve, 800)) // Artificial minimum delay so the UI displays smoothly
    ])
    const deps = await invoke<{ ffmpeg_ready: boolean, ffprobe_ready: boolean }>('check_dependencies', { settings })
    
    if (deps.ffmpeg_ready && deps.ffprobe_ready) {
      globalState.isDependenciesReady = true
    } else {
      globalState.isDependenciesReady = false
    }
  } catch (e) {
    console.error('Failed checking dependencies:', e)
    globalState.isDependenciesReady = false
  } finally {
    globalState.isCheckingDependencies = false
  }
}


async function startAutoDownload() {
  globalState.isDownloadingDependencies = true
  try {
    await invoke('download_ffmpeg')
    await checkDependencies()
  } catch (e) {
    console.error('Download error:', e)
    globalState.downloadStatus = { status: `Error: ${e}`, percentage: 0 }
    globalState.isDownloadingDependencies = false
  }
}

onMounted(async () => {
  unlistenProgress = await listen<{ status: string, percentage: number }>('download_progress', (event) => {
    globalState.downloadStatus = { ...event.payload }
    if (event.payload.status.includes('successfully')) {
      globalState.isDownloadingDependencies = false
    }
  })
  checkDependencies()
})

onUnmounted(() => {
  if (unlistenProgress) unlistenProgress()
})
</script>

<template>
  <header class="border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-background-dark/50 backdrop-blur-md sticky top-0 z-50">
    <div class="max-w-[1200px] mx-auto px-6 h-16 flex items-center justify-between">
      <div class="flex items-center gap-3">
        <span class="material-symbols-outlined text-3xl text-primary">box</span>
        <h1 class="text-xl font-bold tracking-tight">FFBox</h1>
      </div>

      <div class="flex items-center gap-6">
        <nav class="hidden md:flex items-center gap-6 mr-2">
          <router-link 
            to="/" 
            class="text-sm font-medium flex items-center gap-1.5 transition-colors cursor-pointer"
            :class="route.path === '/' ? 'text-primary font-semibold' : 'text-slate-600 dark:text-slate-400 hover:text-primary dark:hover:text-primary'"
          >
            <span class="material-symbols-outlined text-[20px]">dashboard</span>
            Dashboard
          </router-link>
          <router-link 
            to="/settings" 
            class="text-sm font-medium flex items-center gap-1.5 transition-colors cursor-pointer"
            :class="route.path === '/settings' ? 'text-primary font-semibold' : 'text-slate-600 dark:text-slate-400 hover:text-primary dark:hover:text-primary'"
          >
            <span class="material-symbols-outlined text-[20px]">settings</span>
            Settings
          </router-link>
        </nav>
      </div>
    </div>
  </header>
  <router-view v-if="globalState.isDependenciesReady" />

  <!-- Fullscreen Setup Overlay -->
  <main v-else class="flex-1 flex items-center justify-center p-6 sm:p-12 relative overflow-hidden bg-slate-50 dark:bg-slate-900 border-t border-slate-200 dark:border-slate-800">
    <!-- Ambient glowing backgrounds -->
    <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-2xl h-full blur-3xl opacity-30 bg-gradient-to-br from-primary/30 to-purple-500/20 dark:from-primary/10 dark:to-purple-500/10 pointer-events-none"></div>

    <div v-if="globalState.isCheckingDependencies" class="relative z-10 flex flex-col items-center">
      <div class="size-16 border-4 border-slate-200 border-t-primary rounded-full animate-spin mb-6"></div>
      <h2 class="text-2xl font-bold tracking-tight mb-2">Initializing System</h2>
      <p class="text-slate-500 font-medium">Scanning for FFmpeg binaries...</p>
    </div>

    <!-- Missing Dependencies State -->
    <div v-else-if="!globalState.isDownloadingDependencies" class="relative z-10 w-full max-w-lg bg-white dark:bg-slate-800/80 backdrop-blur-sm border border-slate-200 dark:border-slate-700/50 rounded-2xl shadow-xl p-8 sm:p-10 animate-in fade-in zoom-in-95 duration-500">
      <div class="size-20 bg-amber-100 dark:bg-amber-900/30 text-amber-500 rounded-full flex items-center justify-center mb-6 mx-auto shadow-inner">
        <span class="material-symbols-outlined text-4xl">extension</span>
      </div>
      <h2 class="text-3xl font-bold tracking-tight text-center mb-4">FFmpeg Required</h2>
      <p class="text-slate-500 dark:text-slate-400 text-center mb-8 leading-relaxed">
        FFBox relies on the industry-standard FFmpeg engine to perform media conversions faster than light. We noticed it's not installed on your system.
      </p>

      <div class="space-y-4">
        <button 
          @click="startAutoDownload"
          class="w-full bg-primary hover:bg-primary/90 text-white px-6 py-4 rounded-xl font-bold transition-all shadow-lg shadow-primary/20 flex flex-col items-center justify-center gap-1 hover:-translate-y-0.5"
        >
          <span class="flex items-center gap-2"><span class="material-symbols-outlined">download</span> Auto-Install Now</span>
          <span class="text-xs font-medium text-white/70">Downloads ~120MB cross-platform binary</span>
        </button>

        <div class="relative py-4 flex items-center">
          <div class="flex-grow border-t border-slate-200 dark:border-slate-700"></div>
          <span class="flex-shrink-0 mx-4 text-slate-400 text-xs font-bold uppercase tracking-widest">or</span>
          <div class="flex-grow border-t border-slate-200 dark:border-slate-700"></div>
        </div>

        <button 
          @click="() => { globalState.isDependenciesReady = true; router.push('/settings') }"
          class="w-full bg-slate-50 dark:bg-slate-800 hover:bg-slate-100 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 border border-slate-200 dark:border-slate-600 px-6 py-3.5 rounded-xl font-bold transition-colors flex items-center justify-center gap-2"
        >
          <span class="material-symbols-outlined text-[20px]">folder_open</span>
          I already have it (Manual Target)
        </button>
      </div>
    </div>

    <!-- Downloading State -->
    <div v-else class="relative z-10 w-full max-w-lg bg-white dark:bg-slate-800/80 backdrop-blur-sm border border-slate-200 dark:border-slate-700/50 rounded-2xl shadow-xl p-8 sm:p-10 animate-in fade-in slide-in-from-bottom-4 duration-500">
      <div class="flex flex-col items-center text-center">
        <span class="material-symbols-outlined text-5xl text-primary animate-bounce mb-6">cloud_download</span>
        <h2 class="text-2xl font-bold tracking-tight mb-2">Downloading Assets</h2>
        <p class="text-slate-500 font-medium mb-8 max-w-xs">{{ globalState.downloadStatus.status || "Connecting to secure registry..." }}</p>

        <div class="w-full mb-2 flex justify-between items-end">
          <span class="text-xs font-bold text-slate-400 uppercase tracking-widest">Progress</span>
          <span class="text-sm font-bold text-primary">{{ globalState.downloadStatus.percentage.toFixed(1) }}%</span>
        </div>
        <div class="w-full h-3 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden mb-6">
          <div class="bg-primary h-full rounded-full transition-all duration-300 shadow-[0_0_10px_rgba(19,127,236,0.5)]" :style="{ width: `${globalState.downloadStatus.percentage}%` }"></div>
        </div>
        
        <p class="text-xs text-slate-400">Do not close the application.</p>
      </div>
    </div>
  </main>

  <!-- Progress Footer -->
  <footer v-if="globalState.conversionProgress > 0 || globalState.isConverting" class="bg-white dark:bg-slate-900 border-t border-slate-200 dark:border-slate-800 px-6 py-4 mt-auto">
    <div class="max-w-[1200px] mx-auto flex flex-col md:flex-row items-center justify-between gap-4">
      <div class="flex items-center gap-4 w-full md:w-auto">
        <div class="size-10 flex items-center justify-center rounded-full text-white" :class="globalState.conversionStatus.includes('Done') ? 'bg-emerald-500' : 'bg-primary'">
          <span class="material-symbols-outlined" :class="{'animate-spin': globalState.isConverting}">
            {{ globalState.conversionStatus.includes('Done') ? 'check' : 'sync' }}
          </span>
        </div>
        <div class="flex flex-col">
          <p class="text-sm font-bold">Overall Progress</p>
          <p class="text-xs text-slate-500 truncate max-w-[240px]">{{ globalState.conversionStatus }}</p>
        </div>
      </div>
      <div class="flex-1 w-full max-w-lg">
        <div class="flex justify-between items-center mb-1">
          <span class="text-xs font-bold text-primary">{{ globalState.conversionProgress.toFixed(1) }}%</span>
        </div>
        <div class="w-full h-3 bg-slate-100 dark:bg-slate-800 rounded-full overflow-hidden">
          <div class="bg-primary h-full rounded-full transition-all duration-300 shadow-[0_0_10px_rgba(19,127,236,0.5)]" :style="{ width: `${globalState.conversionProgress}%` }"></div>
        </div>
      </div>
    </div>
  </footer>
</template>
