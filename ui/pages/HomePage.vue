<script setup lang="ts">
import { ref, onMounted, computed, onUnmounted, watch, nextTick } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
import { useRouter } from 'vue-router'
import { globalState } from '../store'

const router = useRouter()

// ----------------------------------------------------------------------------
// Types & Interfaces
// ----------------------------------------------------------------------------
interface Preset {
  name: string
  extension: string
  args: string[]
}

interface PresetsConfig {
  presets: Preset[]
}

interface ProgressEvent {
  file: string
  percentage: number
  status: string
}

interface Settings {
  ffmpeg_path: string
  ffprobe_path: string
  default_output_relative: boolean
  default_relative_dir_name: string
  default_custom_output_path: string
  default_enable_suffix: boolean
  default_suffix: string
}

// ----------------------------------------------------------------------------
// Global UI & Stepper State
// ----------------------------------------------------------------------------
const currentStep = ref(1)

const steps = [
  { id: 1, name: 'Input', icon: 'file_open' },
  { id: 2, name: 'Queue', icon: 'format_list_bulleted' },
  { id: 3, name: 'Output', icon: 'tune' },
  { id: 4, name: 'Review', icon: 'fact_check' },
  { id: 5, name: 'Progress', icon: 'sync' }
]

function goToStep(step: number) {
  // Prevent jumping to progress before starting
  if (step === 5 && !globalState.isConverting) return
  
  // Prevent going back if converting
  if (globalState.isConverting) return

  // Prevent jumping forward to empty queue
  if (step > 1 && selectedFiles.value.length === 0) return
  
  // Prevent jumping to review without preset
  if (step > 3 && selectedPresetIndex.value === 'none') {
    alert("Please select a preset first.");
    return
  }

  currentStep.value = step
}

// ----------------------------------------------------------------------------
// Input (Step 1) & Queue (Step 2) State
// ----------------------------------------------------------------------------
const savedPresets = ref<Preset[]>([])
const selectedFiles = ref<File[]>([])
const isDragging = ref(false)

// Sorting
const sortColumn = ref<'name' | 'ext' | null>(null)
const sortAscending = ref(true)

const sortedFiles = computed(() => {
  if (!sortColumn.value) return selectedFiles.value;

  return [...selectedFiles.value].sort((a, b) => {
    let valA = '';
    let valB = '';

    if (sortColumn.value === 'name') {
      valA = a.name.toLowerCase();
      valB = b.name.toLowerCase();
    } else if (sortColumn.value === 'ext') {
      valA = a.name.split('.').pop()?.toLowerCase() || '';
      valB = b.name.split('.').pop()?.toLowerCase() || '';
    }

    if (valA < valB) return sortAscending.value ? -1 : 1;
    if (valA > valB) return sortAscending.value ? 1 : -1;
    return 0;
  });
})

function toggleSort(column: 'name' | 'ext') {
  if (sortColumn.value === column) {
    if (!sortAscending.value) {
      sortColumn.value = null; // reset
      sortAscending.value = true;
    } else {
      sortAscending.value = false;
    }
  } else {
    sortColumn.value = column;
    sortAscending.value = true;
  }
}

import Sortable from 'sortablejs'

// Drag & Drop Queue Reordering (SortableJS)
const queueListRef = ref<HTMLElement | null>(null)
let sortableInstance: Sortable | null = null

function initSortable() {
  if (queueListRef.value && !sortableInstance) {
    sortableInstance = Sortable.create(queueListRef.value, {
      animation: 150,
      handle: '.drag-handle', // Class for the grab icon
      ghostClass: 'bg-primary/5',
      forceFallback: true,    // CRITICAL for Tauri WebView2
      fallbackClass: 'shadow-2xl',
      onEnd: (evt) => {
        if (evt.oldIndex === undefined || evt.newIndex === undefined || evt.oldIndex === evt.newIndex) return;
        
        // 1) Undo DOM manipulation so Vue's virtual DOM patches cleanly
        const item = evt.item;
        const parent = evt.to;
        parent.removeChild(item);
        if (evt.oldIndex < parent.children.length) {
          parent.insertBefore(item, parent.children[evt.oldIndex]);
        } else {
          parent.appendChild(item);
        }

        // 2) Reorder the data using the current visual sorted state
        const newVisualArray = [...sortedFiles.value];
        const [moved] = newVisualArray.splice(evt.oldIndex, 1);
        newVisualArray.splice(evt.newIndex, 0, moved);
        
        selectedFiles.value = newVisualArray;
        // Clear sorting active state so visual matches manual reorder
        sortColumn.value = null; 
      }
    })
  }
}

function destroySortable() {
  if (sortableInstance) {
    sortableInstance.destroy()
    sortableInstance = null
  }
}

watch(currentStep, async (newStep) => {
  if (newStep === 2) {
    await nextTick()
    initSortable()
  }
})
// Output Configuration (Step 3) State
// ----------------------------------------------------------------------------
const selectedPresetIndex = ref('none')

const selectedPresetArgs = computed(() => {
  if (selectedPresetIndex.value === 'none') return ''
  const p = savedPresets.value[parseInt(selectedPresetIndex.value)]
  return p ? p.args.join(' ') : ''
})

const isRelativeOutput = ref(true)
const relativeDirNameInput = ref('')
const customOutputPath = ref('')
const enableSuffix = ref(false)
const suffixInput = ref('')

async function fetchPresets() {
  try {
    const config = await invoke<PresetsConfig>('get_presets')
    savedPresets.value = config.presets

    const lastSavedIndex = localStorage.getItem('lastPresetIndex')
    if (lastSavedIndex && parseInt(lastSavedIndex) < savedPresets.value.length) {
      selectedPresetIndex.value = lastSavedIndex
    }
  } catch (error) {
    console.error('Failed to fetch presets:', error)
  }
}

async function fetchOutputDefaults() {
  try {
    const settings = await invoke<Settings>('get_settings')
    isRelativeOutput.value = settings.default_output_relative
    relativeDirNameInput.value = settings.default_relative_dir_name || 'FFBox'
    customOutputPath.value = settings.default_custom_output_path
    enableSuffix.value = settings.default_enable_suffix
    suffixInput.value = settings.default_suffix
  } catch (err) {
    console.error('Failed to load defaults', err)
  }
}

// Custom in-app confirmation modal (always centered in app window)
const showConfirmModal = ref(false)
let confirmResolve: ((val: boolean) => void) | null = null

function showConfirm(): Promise<boolean> {
  showConfirmModal.value = true
  return new Promise((resolve) => {
    confirmResolve = resolve
  })
}
function onConfirmYes() {
  showConfirmModal.value = false
  confirmResolve?.(true)
}
function onConfirmNo() {
  showConfirmModal.value = false
  confirmResolve?.(false)
}

async function confirmPresetNavigation() {
  const confirmed = await showConfirm()
  if (confirmed) {
    router.push('/settings')
  }
}

async function browseOutputFolder() {
  try {
    const folderPath = await open({
      directory: true,
      multiple: false,
      title: 'Select Output Folder'
    })
    if (folderPath && !Array.isArray(folderPath)) customOutputPath.value = folderPath
  } catch (err) { console.error(err) }
}


// ----------------------------------------------------------------------------
// File Browsing & OS Drag-Drop Handlers
// ----------------------------------------------------------------------------
async function processPaths(paths: string[]) {
  if (paths.length === 0) return
  try {
    const expandedPaths: string[] = await invoke('expand_media_paths', { paths })
    expandedPaths.forEach((path) => {
      const isDuplicate = selectedFiles.value.some(f => (f as any).path === path || f.name === path)
      if (!isDuplicate) {
        const fileObj = {
          name: path.split(/[\\/]/).pop() || path,
          path: path
        } as unknown as File
        selectedFiles.value.push(fileObj)
      }
    })
    if (selectedFiles.value.length > 0 && currentStep.value === 1) {
       currentStep.value = 2; // Auto advance to queue
    }
  } catch (err) {
    console.error('Failed to expand paths', err)
  }
}

async function browseFiles() {
  try {
    const filePaths = await open({
      multiple: true,
      title: 'Select Media Files',
      filters: [
        { name: 'Media', extensions: ['mp4', 'mkv', 'avi', 'mov', 'mp3', 'wav', 'flac'] }
      ]
    })
    if (filePaths) {
      const paths = Array.isArray(filePaths) ? filePaths : [filePaths]
      await processPaths(paths)
    }
  } catch (err) {
    console.error('Failed to open file dialog', err)
  }
}

async function browseFolders() {
  try {
    const folderPaths = await open({
      multiple: true,
      directory: true,
      title: 'Select Folders'
    })
    if (folderPaths) {
      const paths = Array.isArray(folderPaths) ? folderPaths : [folderPaths]
      await processPaths(paths)
    }
  } catch (err) {
    console.error('Failed to open folder dialog', err)
  }
}

const onOsDragOver = (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = true
}
const onOsDragLeave = () => {
  isDragging.value = false
}
const onOsDrop = async (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = false
  if (e.dataTransfer && e.dataTransfer.files) {
    const paths = Array.from(e.dataTransfer.files).map(f => (f as any).path || f.name)
    await processPaths(paths)
  }
}

function removeFile(fileToRemove: File) {
  selectedFiles.value = selectedFiles.value.filter(f => f.name !== fileToRemove.name)
  if (selectedFiles.value.length === 0) {
    currentStep.value = 1
  }
}

function clearAllFiles() {
  selectedFiles.value = []
  currentStep.value = 1
}


// ----------------------------------------------------------------------------
// Conversion Logic
// ----------------------------------------------------------------------------
function handlePresetChange() {
  if (selectedPresetIndex.value !== 'none') {
    localStorage.setItem('lastPresetIndex', selectedPresetIndex.value)
  }
}

async function startConversion() {
  if (globalState.isConverting || selectedFiles.value.length === 0 || selectedPresetIndex.value === 'none') return

  const selectedPreset = savedPresets.value[parseInt(selectedPresetIndex.value)]
  if (!selectedPreset) return

  // Need to ensure the output directory actually exists or prompt if custom and empty
  if (!isRelativeOutput.value && !customOutputPath.value) {
    alert("Please select a custom output folder.");
    goToStep(3);
    return;
  }

  const filePaths = selectedFiles.value.map(f => (f as any).path || f.name)
  if (filePaths.length === 0) {
    alert('No valid file paths found.')
    return
  }

  globalState.isConverting = true
  globalState.conversionStatus = 'Conversion started...'
  globalState.conversionProgress = 0
  
  // Lock step to progress
  currentStep.value = 5

  try {
    await invoke('start_conversion', {
      payload: { 
        files: filePaths, 
        preset: selectedPreset,
        output_config: {
          is_relative: isRelativeOutput.value,
          relative_dir_name: relativeDirNameInput.value.trim() || 'FFBox',
          custom_dir_path: isRelativeOutput.value ? null : customOutputPath.value,
          apply_suffix: enableSuffix.value,
          name_suffix: enableSuffix.value ? suffixInput.value : null
        }
      }
    })
    console.log('All conversions finished backend loop.')
  } catch (err) {
    console.error(err)
    globalState.conversionStatus = `Error: ${err}`
    globalState.isConverting = false
  }
}

let unlisten: () => void

onMounted(async () => {
  fetchPresets()
  fetchOutputDefaults()

  unlisten = await listen<ProgressEvent>('conversion_progress', (event) => {
    const { file, percentage, status } = event.payload
    globalState.conversionProgress = percentage
    globalState.conversionStatus = status
    globalState.currentFileProgress = file

    if (percentage >= 100 || status.includes('failed') || status.includes('Done')) {
      globalState.isConverting = false
    }
  })
})

onUnmounted(() => {
  if (unlisten) unlisten()
  destroySortable()
})
</script>

<template>
  <main class="flex-1 max-w-[1200px] mx-auto w-full p-6 flex flex-col gap-8">
    
    <!-- Stepper Navigation -->
    <div class="w-full flex justify-between relative px-2 sm:px-12 mt-4 max-w-4xl mx-auto">
      <div 
        class="absolute top-1/2 left-10 right-10 h-1 bg-slate-200 dark:bg-slate-800 -z-10 -translate-y-1/2 rounded-full"
      >
         <div 
           class="h-full bg-primary transition-all duration-500 ease-in-out"
           :style="{ width: `${((currentStep - 1) / (steps.length - 1)) * 100}%` }"
         ></div>
      </div>

      <button 
        v-for="step in steps" 
        :key="step.id"
        @click="goToStep(step.id)"
        class="flex flex-col items-center gap-2 group outline-none"
        :class="[
          globalState.isConverting ? 'cursor-not-allowed opacity-70' : 'cursor-pointer',
          currentStep === step.id ? 'opacity-100' : 'opacity-60 hover:opacity-100'
        ]"
      >
        <div 
          class="w-10 h-10 rounded-full flex items-center justify-center transition-all duration-300 group-hover:scale-110 shadow-sm"
          :class="[
            currentStep >= step.id ? 'bg-primary text-white shadow-primary/30' : 'bg-slate-100 dark:bg-slate-800 text-slate-400 border-2 border-slate-200 dark:border-slate-700 w-10 h-10' 
          ]"
        >
          <span class="material-symbols-outlined text-[20px]">{{ step.icon }}</span>
        </div>
        <span 
          class="text-xs font-bold tracking-wider uppercase transition-colors"
          :class="currentStep >= step.id ? 'text-primary' : 'text-slate-500'"
        >
          {{ step.name }}
        </span>
      </button>
    </div>

    <div class="bg-white dark:bg-slate-900/50 rounded-xl border border-slate-200 dark:border-slate-800 overflow-hidden shadow-sm flex-1 flex flex-col min-h-[500px]">
      
      <!-- STEP 1: INPUT -->
      <section v-show="currentStep === 1" class="flex-1 p-8 md:p-12 flex flex-col items-center justify-center animate-in fade-in slide-in-from-bottom-4 duration-300">
        
        <!-- Empty State -->
        <div v-if="selectedFiles.length === 0"
          class="w-full max-w-2xl border-2 border-dashed rounded-2xl p-16 flex flex-col items-center justify-center text-center group transition-all"
          :class="isDragging ? 'border-primary bg-primary/5' : 'border-slate-300 dark:border-slate-700 hover:border-primary dark:hover:border-primary cursor-pointer bg-slate-50/50 dark:bg-slate-800/20'"
          @dragover="onOsDragOver" 
          @dragleave="onOsDragLeave" 
          @drop="onOsDrop"
          @click.self="browseFiles"
        >
          <div class="bg-primary/10 text-primary p-6 rounded-full mb-6 group-hover:scale-110 transition-transform pointer-events-none">
            <span class="material-symbols-outlined text-5xl">upload_file</span>
          </div>
          <h2 class="text-3xl font-bold mb-3 pointer-events-none tracking-tight">Drop Media Here</h2>
          <p class="text-slate-500 dark:text-slate-400 max-w-sm mx-auto mb-10 pointer-events-none">
            Support for MP4, MKV, MOV, AVI, MP3 and more. Batch processor auto-detects thousands of files instantly.
          </p>
          <div class="flex flex-col sm:flex-row gap-4">
            <button @click.stop="browseFiles" class="bg-primary hover:bg-primary/90 text-white px-8 py-3.5 rounded-xl font-bold transition-all shadow-lg shadow-primary/20 flex items-center gap-2 hover:-translate-y-0.5">
              <span class="material-symbols-outlined">add_circle</span>
              Browse Files
            </button>
            <button @click.stop="browseFolders" class="bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-700 dark:text-slate-200 border border-slate-200 dark:border-slate-700 px-8 py-3.5 rounded-xl font-bold transition-all shadow-sm flex items-center gap-2 hover:-translate-y-0.5">
              <span class="material-symbols-outlined">folder</span>
              Browse Folders
            </button>
          </div>
        </div>

        <!-- Populated State (Navigated Back to Step 1) -->
        <div v-else class="w-full max-w-2xl bg-amber-50 dark:bg-amber-900/10 border border-amber-200 dark:border-amber-900/50 rounded-2xl p-16 flex flex-col items-center justify-center text-center transition-all">
          <div class="bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-500 p-6 rounded-full mb-6">
            <span class="material-symbols-outlined text-5xl">warning</span>
          </div>
          <h2 class="text-3xl font-bold mb-3 tracking-tight text-amber-900 dark:text-amber-500">Queue Already Populated</h2>
          <p class="text-amber-700 dark:text-amber-600/80 max-w-sm mx-auto mb-10 font-medium">
            You currently have <strong>{{ selectedFiles.length }}</strong> files loaded in your active session. You can continue configuring their output, or discard them to start completely fresh.
          </p>
          <div class="flex flex-col sm:flex-row gap-4 w-full sm:w-auto">
             <button @click="clearAllFiles" class="bg-white dark:bg-slate-800 hover:bg-red-50 hover:text-red-600 hover:border-red-200 dark:hover:bg-red-900/20 dark:hover:border-red-900/50 dark:hover:text-red-400 text-slate-700 dark:text-slate-200 border border-slate-200 dark:border-slate-700 px-8 py-3.5 rounded-xl font-bold transition-all shadow-sm flex items-center justify-center gap-2 flex-1">
              <span class="material-symbols-outlined">delete_sweep</span>
              Clear Queue
            </button>
            <button @click="goToStep(2)" class="bg-amber-500 hover:bg-amber-600 text-white px-8 py-3.5 rounded-xl font-bold transition-all shadow-lg shadow-amber-500/20 flex items-center justify-center gap-2 flex-1 relative overflow-hidden flex-shrink-0 min-w-[200px]">
              <span class="material-symbols-outlined relative z-10">fast_forward</span>
              <span class="relative z-10">Return to Queue</span>
            </button>
          </div>
        </div>
      </section>

      <!-- STEP 2: QUEUE -->
      <section v-show="currentStep === 2" class="flex-1 flex flex-col animate-in fade-in zoom-in-95 duration-300">
        <div class="px-6 py-5 border-b border-slate-200 dark:border-slate-800 flex justify-between items-center bg-slate-50/50 dark:bg-slate-800/30">
          <div class="flex items-center gap-3">
            <h3 class="font-bold text-lg">Processing Queue</h3>
            <span class="bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300 text-xs font-bold px-2 py-0.5 rounded-md">{{ selectedFiles.length }} files</span>
          </div>
          <div class="flex items-center gap-4">
             <button @click="browseFiles" class="text-sm font-semibold text-primary hover:text-primary/80 transition-colors flex items-center gap-1">
              <span class="material-symbols-outlined text-[18px]">add</span> Add More
             </button>
             <button @click="clearAllFiles" class="text-xs font-bold text-slate-500 hover:text-red-500 uppercase tracking-wider transition-colors">Clear All</button>
          </div>
        </div>
        
        <!-- Queue Table Header -->
        <div class="grid grid-cols-[auto_1fr_auto] gap-4 px-6 py-3 border-b border-slate-200 dark:border-slate-800 bg-slate-100/50 dark:bg-slate-900/50 text-xs font-bold text-slate-500 uppercase tracking-wider select-none">
          <div class="w-8"></div> <!-- Drag Handle Space -->
          <div class="flex items-center gap-2 cursor-pointer hover:text-slate-700 dark:hover:text-slate-300 w-fit" @click="toggleSort('name')">
            Filename
            <span v-if="sortColumn === 'name'" class="material-symbols-outlined text-[14px]">{{ sortAscending ? 'arrow_upward' : 'arrow_downward' }}</span>
          </div>
          <div class="flex items-center gap-8">
            <div class="flex items-center gap-2 cursor-pointer hover:text-slate-700 dark:hover:text-slate-300" @click="toggleSort('ext')">
              Type
              <span v-if="sortColumn === 'ext'" class="material-symbols-outlined text-[14px]">{{ sortAscending ? 'arrow_upward' : 'arrow_downward' }}</span>
            </div>
            <div class="w-8 opacity-0">Act</div> <!-- Actions space -->
          </div>
        </div>

        <div class="flex-1 overflow-y-auto min-h-[300px]">
          <div ref="queueListRef" class="divide-y divide-slate-100 dark:divide-slate-800/50">
            <div v-for="file in sortedFiles" :key="(file as any).path || file.name" 
                 class="px-6 py-3 flex items-center gap-4 group hover:bg-slate-50 dark:hover:bg-slate-800/50 transition-colors"
                 :class="{'bg-primary/5': globalState.currentFileProgress.includes(file.name) && currentStep === 5}"
            >
              <span class="material-symbols-outlined text-slate-300 dark:text-slate-600 group-hover:text-slate-400 cursor-grab active:cursor-grabbing drag-handle">drag_indicator</span>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold truncate">{{ file.name }}</p>
              </div>
              <div class="flex items-center gap-8">
                <span class="text-xs font-mono text-slate-400 bg-slate-100 dark:bg-slate-800 px-2 py-1 rounded">{{ file.name.split('.').pop()?.toUpperCase() }}</span>
                <button @click="removeFile(file)" class="text-slate-400 hover:text-red-500 transition-colors w-8 flex justify-end">
                  <span class="material-symbols-outlined text-[20px]">delete</span>
                </button>
              </div>
            </div>
          </div>
        </div>

        <div class="p-6 border-t border-slate-200 dark:border-slate-800 flex justify-between items-center bg-slate-50/50 dark:bg-slate-800/30">
          <p class="text-xs text-slate-500 flex items-center gap-1"><span class="material-symbols-outlined text-[14px]">info</span> Drag rows or click column headers to reorder</p>
          <button @click="currentStep = 3" class="bg-primary hover:bg-primary/90 text-white px-8 py-2.5 rounded-lg font-bold transition-colors shadow-lg shadow-primary/20 flex items-center gap-2">
            Next: Output <span class="material-symbols-outlined text-[18px]">arrow_forward</span>
          </button>
        </div>
      </section>

      <!-- STEP 3: OUTPUT -->
      <section v-show="currentStep === 3" class="flex-1 flex flex-col animate-in fade-in slide-in-from-right-4 duration-300 bg-slate-50/30 dark:bg-slate-900/20">
        <div class="p-8 pb-4 border-b border-slate-200 dark:border-slate-800">
           <h3 class="text-xl font-bold tracking-tight mb-2">Configure Conversion</h3>
           <p class="text-sm text-slate-500 max-w-lg">These overrides apply only to this session. Default behaviors can be permanently changed in Global Settings.</p>
        </div>

        <div class="flex-1 p-8 grid grid-cols-1 md:grid-cols-2 gap-8 overflow-y-auto">
          <!-- Left Column (Preset) -->
          <div class="space-y-6">
            <div class="space-y-3">
              <div class="flex justify-between items-center">
                <label class="text-sm font-bold text-slate-700 dark:text-slate-300 flex items-center gap-2">
                  <span class="material-symbols-outlined text-primary">tune</span> Encoding Preset
                </label>
                <button @click="confirmPresetNavigation" class="text-xs text-primary font-bold hover:underline">Manage Presets</button>
              </div>
              <div class="relative">
                <select v-model="selectedPresetIndex" @change="handlePresetChange" class="w-full bg-white dark:bg-slate-800 border shadow-sm border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 appearance-none focus:ring-2 focus:ring-primary focus:border-transparent outline-none cursor-pointer text-sm font-medium">
                  <option value="none" disabled>Select a preset...</option>
                  <option v-for="(preset, index) in savedPresets" :key="index" :value="String(index)">
                    {{ preset.name }} (→ .{{ preset.extension }})
                  </option>
                </select>
              </div>
              <div v-if="selectedPresetArgs" class="text-xs text-slate-500 mt-2 p-4 bg-slate-100 dark:bg-slate-800/80 border border-slate-200 dark:border-slate-700 rounded-lg font-mono break-words leading-relaxed max-h-[120px] overflow-y-auto shadow-inner">
                {{ selectedPresetArgs }}
              </div>
            </div>

            <!-- Parallel Workers upcoming block -->
            <div class="mt-8 p-5 rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 opacity-60 cursor-not-allowed group relative">
               <div class="flex justify-between items-center mb-2">
                 <label class="text-sm font-bold text-slate-500 dark:text-slate-400 flex items-center gap-2">
                   <span class="material-symbols-outlined">network_node</span> Parallel Workers
                 </label>
                 <span class="text-[10px] uppercase font-bold tracking-wider text-slate-400 border border-slate-300 dark:border-slate-600 px-2 py-0.5 rounded">Soon</span>
               </div>
               <p class="text-xs text-slate-400">Run multiple encodes simultaneously.</p>
               
               <!-- Tooltip -->
               <div class="absolute -top-10 left-1/2 -translate-x-1/2 bg-slate-800 text-white text-xs px-3 py-1.5 rounded opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none whitespace-nowrap shadow-xl">
                 Available in the upcoming version
                 <div class="absolute -bottom-1 left-1/2 -translate-x-1/2 w-2 h-2 bg-slate-800 rotate-45"></div>
               </div>
            </div>
          </div>

          <!-- Right Column (Folder/Name) -->
          <div class="space-y-8 pl-0 md:pl-8 md:border-l border-slate-200 dark:border-slate-800">
             <div class="space-y-4">
              <label class="text-sm font-bold text-slate-700 dark:text-slate-300 flex items-center gap-2 mb-2">
                <span class="material-symbols-outlined text-primary">folder_open</span> Output Destination
              </label>
              <div class="flex flex-col gap-3">
                <label class="flex items-center gap-3 cursor-pointer p-3 rounded-lg border transition-colors" :class="isRelativeOutput ? 'border-primary bg-primary/5' : 'border-slate-200 dark:border-slate-700 hover:border-primary/50'">
                  <input type="radio" :value="true" v-model="isRelativeOutput" class="text-primary focus:ring-primary w-4 h-4">
                  <span class="text-sm font-semibold">Relative (FFBox Subfolder)</span>
                </label>
                <div v-show="isRelativeOutput" class="flex gap-2 animate-in fade-in slide-in-from-top-2">
                  <input v-model="relativeDirNameInput" type="text" placeholder="e.g. FFBox" class="flex-1 bg-white dark:bg-slate-800 border shadow-inner border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 text-sm focus:ring-2 focus:ring-primary outline-none">
                </div>
                <label class="flex items-center gap-3 cursor-pointer p-3 rounded-lg border transition-colors" :class="!isRelativeOutput ? 'border-primary bg-primary/5' : 'border-slate-200 dark:border-slate-700 hover:border-primary/50'">
                  <input type="radio" :value="false" v-model="isRelativeOutput" class="text-primary focus:ring-primary w-4 h-4">
                  <span class="text-sm font-semibold">Custom Directory</span>
                </label>
                <div v-show="!isRelativeOutput" class="flex gap-2 animate-in fade-in slide-in-from-top-2">
                  <input v-model="customOutputPath" type="text" placeholder="Select destination..." readonly class="flex-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-3 py-2 text-sm outline-none cursor-default opacity-70">
                  <button @click="browseOutputFolder" class="bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 px-4 py-2 rounded-lg text-sm font-semibold transition-colors">Browse</button>
                </div>
              </div>              
            </div>

            <hr class="border-slate-200 dark:border-slate-800">

            <div class="space-y-4">
              <div class="flex items-center justify-between mt-2">
                <label class="text-sm font-bold text-slate-700 dark:text-slate-300 flex items-center gap-2">
                  <span class="material-symbols-outlined text-primary">edit_document</span> Append Naming Suffix
                </label>
                <button 
                  @click="enableSuffix = !enableSuffix"
                  class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none"
                  :class="enableSuffix ? 'bg-primary' : 'bg-slate-300 dark:bg-slate-600'"
                >
                  <span 
                    class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform"
                    :class="enableSuffix ? 'translate-x-6' : 'translate-x-1'"
                  />
                </button>
              </div>
              
              <div v-show="enableSuffix" class="animate-in fade-in slide-in-from-top-2">
                <input v-model="suffixInput" type="text" placeholder="e.g. _converted" class="w-full bg-white dark:bg-slate-800 border shadow-inner border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 text-sm focus:ring-2 focus:ring-primary outline-none font-mono">
              </div>
            </div>
          </div>
        </div>

        <div class="p-6 border-t border-slate-200 dark:border-slate-800 flex justify-between items-center bg-white dark:bg-slate-800/30">
          <button @click="currentStep = 2" class="text-sm font-bold text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 transition-colors px-4 py-2">
             Back
          </button>
          <button @click="goToStep(4)" :disabled="selectedPresetIndex === 'none'" class="bg-primary hover:bg-primary/90 text-white px-8 py-2.5 rounded-lg font-bold transition-all shadow-lg shadow-primary/20 flex items-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed cursor-pointer">
            Next: Review <span class="material-symbols-outlined text-[18px]">rule</span>
          </button>
        </div>
      </section>

      <!-- STEP 4: REVIEW -->
      <section v-show="currentStep === 4" class="flex-1 flex flex-col items-center justify-center p-8 animate-in fade-in zoom-in-95 duration-300">
        <div class="w-full max-w-lg bg-slate-50 dark:bg-slate-800 rounded-2xl border border-slate-200 dark:border-slate-700 overflow-hidden shadow-xl mb-8">
           <div class="bg-primary p-6 text-white text-center">
             <span class="material-symbols-outlined text-5xl mb-2">rocket_launch</span>
             <h3 class="text-2xl font-bold">Ready to Start</h3>
           </div>
           
           <div class="p-8 space-y-6">
             <div class="flex justify-between items-center border-b border-slate-200 dark:border-slate-700 pb-4">
               <span class="text-slate-500 font-medium text-sm">Target Preset</span>
               <span class="font-bold bg-slate-200 dark:bg-slate-900 px-3 py-1 rounded text-sm">{{ selectedPresetIndex !== 'none' && savedPresets[parseInt(selectedPresetIndex)] ? savedPresets[parseInt(selectedPresetIndex)].name : 'None' }}</span>
             </div>
             
             <div class="flex justify-between items-center border-b border-slate-200 dark:border-slate-700 pb-4">
               <span class="text-slate-500 font-medium text-sm">Media Files</span>
               <span class="font-bold text-lg">{{ selectedFiles.length }}</span>
             </div>

             <div class="flex justify-between items-center pb-2">
               <span class="text-slate-500 font-medium text-sm">Destination</span>
               <div class="text-right">
                 <p class="font-bold text-sm">{{ isRelativeOutput ? `Relative ${relativeDirNameInput || 'FFBox'}/` : 'Custom Directory' }}</p>
                 <p v-if="enableSuffix" class="text-xs text-primary font-mono mt-0.5">Appends: {{ suffixInput }}</p>
               </div>
             </div>
           </div>
        </div>

        <div class="flex items-center gap-6">
          <button @click="currentStep = 3" class="text-sm font-bold text-slate-500 hover:text-slate-800 dark:hover:text-slate-200 transition-colors px-4 py-2">
             Back to Config
          </button>
          
          <button 
            @click="startConversion"
            :disabled="globalState.isConverting"
            class="bg-emerald-500 hover:bg-emerald-600 text-white px-10 py-4 rounded-xl font-bold text-lg transition-all shadow-xl shadow-emerald-500/30 flex items-center gap-3 hover:-translate-y-1"
          >
            <span class="material-symbols-outlined text-2xl">check_circle</span>
            Start Conversion Engine
          </button>
        </div>
      </section>

      <!-- STEP 5: PROGRESS -->
      <section v-show="currentStep === 5" class="flex-1 flex flex-col items-center justify-center p-8 animate-in fade-in duration-500">
         <div v-if="!globalState.isConverting && globalState.conversionProgress >= 100" class="flex flex-col items-center text-center animate-in zoom-in slide-in-from-bottom-4">
            <div class="size-24 bg-emerald-100 dark:bg-emerald-900/30 text-emerald-500 rounded-full flex items-center justify-center mb-6 shadow-xl shadow-emerald-500/20">
              <span class="material-symbols-outlined text-5xl">task_alt</span>
            </div>
            <h2 class="text-3xl font-bold mb-2">Batch Complete!</h2>
            <p class="text-slate-500 max-w-sm mb-8">All your files have been successfully processed and saved to your requested output destination.</p>
            
            <button @click="clearAllFiles" class="bg-primary hover:bg-primary/90 text-white px-8 py-3 rounded-xl font-bold shadow-lg shadow-primary/20 transition-all flex items-center gap-2">
              <span class="material-symbols-outlined">restart_alt</span> Start New Batch
            </button>
         </div>

         <div v-else class="w-full max-w-2xl text-center">
            <div class="mb-12 relative w-32 h-32 mx-auto">
              <svg class="animate-spin text-primary w-full h-full" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                <circle class="opacity-20" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
              </svg>
              <div class="absolute inset-0 flex items-center justify-center">
                <span class="text-2xl font-bold text-primary">{{ globalState.conversionProgress.toFixed(0) }}%</span>
              </div>
            </div>

            <div class="bg-slate-50 dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700 rounded-xl p-6 shadow-inner">
               <p class="text-sm font-bold text-slate-500 uppercase tracking-widest mb-2">Processing Phase</p>
               <p class="text-xl font-medium tracking-tight h-8 truncate px-4">{{ globalState.conversionStatus }}</p>
               
               <div class="mt-6 text-sm text-slate-400 flex items-center justify-center gap-2">
                 <span class="material-symbols-outlined text-[18px]">movie</span>
                 <span class="truncate max-w-[300px]">{{ globalState.currentFileProgress || "Preparing files..." }}</span>
               </div>
            </div>
         </div>
      </section>

    </div>
  </main>

  <!-- Custom Confirmation Modal -->
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="showConfirmModal" class="fixed inset-0 z-50 flex items-center justify-center">
        <!-- Backdrop -->
        <div class="absolute inset-0 bg-black/50 backdrop-blur-sm" @click="onConfirmNo"></div>
        <!-- Dialog -->
        <div class="relative bg-white dark:bg-slate-800 rounded-2xl shadow-2xl border border-slate-200 dark:border-slate-700 w-full max-w-md mx-4 overflow-hidden animate-in zoom-in-95 fade-in duration-200">
          <div class="p-6 flex gap-4 items-start">
            <div class="shrink-0 w-10 h-10 rounded-full bg-amber-100 dark:bg-amber-900/40 flex items-center justify-center">
              <span class="material-symbols-outlined text-amber-500 text-xl">warning</span>
            </div>
            <div>
              <h3 class="text-base font-bold text-slate-800 dark:text-slate-100 mb-1">Leave current session?</h3>
              <p class="text-sm text-slate-500 dark:text-slate-400">Navigating to Settings will reset your current conversion session. Any configuration will be lost.</p>
            </div>
          </div>
          <div class="px-6 pb-5 flex justify-end gap-3">
            <button @click="onConfirmNo" class="px-4 py-2 text-sm font-semibold rounded-lg border border-slate-200 dark:border-slate-600 text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-700/50 transition-colors">
              Cancel
            </button>
            <button @click="onConfirmYes" class="px-4 py-2 text-sm font-semibold rounded-lg bg-amber-500 hover:bg-amber-600 text-white transition-colors shadow-lg shadow-amber-500/20">
              Continue to Settings
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* Stepper Transitions Details */
.fade-in {
  animation: fadeIn 0.3s ease-in-out;
}

@keyframes fadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}
</style>
