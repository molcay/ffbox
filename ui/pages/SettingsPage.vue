<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { useRouter } from 'vue-router'

interface Settings {
  ffmpeg_path: string
  ffprobe_path: string
  default_output_relative: boolean
  default_relative_dir_name: string
  default_custom_output_path: string
  default_enable_suffix: boolean
  default_suffix: string
}

interface Preset {
  name: string
  extension: string
  args: string[]
}

interface PresetsConfig {
  presets: Preset[]
}

const router = useRouter()

// General Settings State
const ffmpegPathInput = ref('')
const ffprobePathInput = ref('')
const isRelativeOutput = ref(true)
const relativeDirNameInput = ref('')
const customOutputPath = ref('')
const enableSuffix = ref(false)
const suffixInput = ref('')

// Presets State
const presets = ref<Preset[]>([])
const showPresetModal = ref(false)
const editingPresetIndex = ref<number | null>(null)
const presetForm = ref({ name: '', extension: '', args: '' })

// UI State
const activeTab = ref<'global' | 'presets'>('global')

async function loadSettings() {
  try {
    const settings = await invoke<Settings>('get_settings')
    ffmpegPathInput.value = settings.ffmpeg_path !== 'ffmpeg' ? settings.ffmpeg_path : ''
    ffprobePathInput.value = settings.ffprobe_path !== 'ffprobe' ? settings.ffprobe_path : ''
    isRelativeOutput.value = settings.default_output_relative
    relativeDirNameInput.value = settings.default_relative_dir_name || 'FFBox'
    customOutputPath.value = settings.default_custom_output_path
    enableSuffix.value = settings.default_enable_suffix
    suffixInput.value = settings.default_suffix
  } catch (error) {
    console.error('Failed to load settings:', error)
  }

  try {
    const savedPresets = await invoke<PresetsConfig>('get_presets')
    presets.value = savedPresets.presets
  } catch (error) {
    console.error('Failed to load presets:', error)
  }
}

async function saveSettings() {
  const newSettings: Settings = {
    ffmpeg_path: ffmpegPathInput.value.trim() || 'ffmpeg',
    ffprobe_path: ffprobePathInput.value.trim() || 'ffprobe',
    default_output_relative: isRelativeOutput.value,
    default_relative_dir_name: relativeDirNameInput.value.trim() || 'FFBox',
    default_custom_output_path: customOutputPath.value.trim(),
    default_enable_suffix: enableSuffix.value,
    default_suffix: suffixInput.value.trim()
  }

  try {
    await invoke('save_settings', { settings: newSettings })
    await invoke('save_presets', { config: { presets: presets.value } })
    router.push('/')
  } catch (error) {
    console.error('Failed to save settings:', error)
    alert('Failed to save settings.')
  }
}

// Preset Handlers
function openPresetModal(index?: number) {
  if (index !== undefined) {
    editingPresetIndex.value = index
    const p = presets.value[index]
    presetForm.value = { 
      name: p.name, 
      extension: p.extension, 
      args: p.args.join(' ') 
    }
  } else {
    editingPresetIndex.value = null
    presetForm.value = { name: '', extension: '', args: '' }
  }
  showPresetModal.value = true
}

function savePreset() {
  if (!presetForm.value.name.trim() || !presetForm.value.extension.trim()) {
    alert('Name and Extension are required.')
    return
  }

  // Parse space-separated arguments, maintaining quoted strings block
  const rawArgs = presetForm.value.args.trim()
  const argMatches = rawArgs.match(/(?:[^\s"]+|"[^"]*")+/g) || []
  const parsedArgs = argMatches.map(a => a.replace(/^"|"$/g, ''))

  const newPreset: Preset = {
    name: presetForm.value.name.trim(),
    extension: presetForm.value.extension.replace(/^\./, '').trim(), // Ensure no leading dot
    args: parsedArgs
  }

  if (editingPresetIndex.value !== null) {
    presets.value[editingPresetIndex.value] = newPreset
  } else {
    presets.value.push(newPreset)
  }
  
  // Auto-save presets immediately
  invoke('save_presets', { config: { presets: presets.value } })
    .catch(err => console.error("Failed to auto-save preset:", err))

  showPresetModal.value = false
}

function deletePreset(index: number) {
  if (confirm('Are you sure you want to delete this preset?')) {
    presets.value.splice(index, 1)
    
    // Auto-save presets immediately
    invoke('save_presets', { config: { presets: presets.value } })
      .catch(err => console.error("Failed to auto-save preset deletion:", err))
  }
}


async function browseFfmpeg() {
  try {
    const filePath = await open({
      multiple: false,
      title: 'Select FFmpeg Executable',
      filters: [{ name: 'Executables', extensions: ['exe'] }]
    })
    if (filePath && !Array.isArray(filePath)) ffmpegPathInput.value = filePath
  } catch (err) { console.error(err) }
}

async function browseFfprobe() {
  try {
    const filePath = await open({
      multiple: false,
      title: 'Select FFprobe Executable',
      filters: [{ name: 'Executables', extensions: ['exe'] }]
    })
    if (filePath && !Array.isArray(filePath)) ffprobePathInput.value = filePath
  } catch (err) { console.error(err) }
}

async function browseOutputFolder() {
  try {
    const folderPath = await open({
      directory: true,
      multiple: false,
      title: 'Select Default Output Folder'
    })
    if (folderPath && !Array.isArray(folderPath)) customOutputPath.value = folderPath
  } catch (err) { console.error(err) }
}

onMounted(() => {
  loadSettings()
})
</script>

<template>
  <main class="flex-1 max-w-[1200px] mx-auto w-full p-6">
    <div class="bg-white dark:bg-slate-900/50 border border-slate-200 dark:border-slate-800 rounded-xl shadow-sm p-8 max-w-2xl mx-auto">
      <div class="mb-8 border-b border-slate-200 dark:border-slate-800 pb-4">
        <h2 class="text-2xl font-bold flex items-center gap-2">
          <span class="material-symbols-outlined text-primary">settings</span>
          Application Settings
        </h2>
        <p class="text-sm text-slate-500 dark:text-slate-400 mt-2">
          Configure dependencies and default behaviors for FFBox.
        </p>
      </div>
      
      <div class="mb-6 flex gap-6 border-b border-slate-200 dark:border-slate-800">
        <button 
          @click="activeTab = 'global'" 
          class="pb-3 text-sm font-semibold transition-colors border-b-2"
          :class="activeTab === 'global' ? 'border-primary text-primary' : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'"
        >
          Global Configuration
        </button>
        <button 
          @click="activeTab = 'presets'" 
          class="pb-3 text-sm font-semibold transition-colors border-b-2"
          :class="activeTab === 'presets' ? 'border-primary text-primary' : 'border-transparent text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'"
        >
          Presets Management
        </button>
      </div>
      
      <div class="space-y-8 relative">
        
        <!-- Global Tab pane -->
        <div v-show="activeTab === 'global'" class="space-y-8 animate-in fade-in slide-in-from-bottom-2 duration-300">
          <!-- Output Defaults -->
        <div class="space-y-4">
          <h3 class="text-lg font-semibold text-slate-700 dark:text-slate-300">Default Output Configuration</h3>
          <p class="text-sm text-slate-500 dark:text-slate-400">
            Set the default behavior for file destinations and naming formats.
          </p>

          <div class="space-y-4 bg-slate-50 dark:bg-slate-800/50 p-5 rounded-lg border border-slate-200 dark:border-slate-700/50">
            <!-- Folder Selection -->
            <div class="space-y-3">
              <label class="text-sm font-semibold text-slate-600 dark:text-slate-400">Conversion Output Folder</label>
              <div class="flex gap-4">
                <label class="flex items-center gap-2 cursor-pointer">
                  <input type="radio" :value="true" v-model="isRelativeOutput" class="text-primary focus:ring-primary w-4 h-4">
                  <span class="text-sm font-medium">Relative (FFBox Subfolder)</span>
                </label>
                <label class="flex items-center gap-2 cursor-pointer">
                  <input type="radio" :value="false" v-model="isRelativeOutput" class="text-primary focus:ring-primary w-4 h-4">
                  <span class="text-sm font-medium">Custom Folder</span>
                </label>
              </div>
              
              <div v-if="isRelativeOutput" class="flex gap-2 pt-2 animate-in fade-in slide-in-from-top-2">
                <input v-model="relativeDirNameInput" type="text" placeholder="e.g. FFBox" class="flex-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-2 focus:ring-primary outline-none">
              </div>

              <div v-if="!isRelativeOutput" class="flex gap-2 pt-2 animate-in fade-in slide-in-from-top-2">
                <input v-model="customOutputPath" type="text" placeholder="e.g. C:\Users\Username\Videos" class="flex-1 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-2 focus:ring-primary outline-none">
                <button @click="browseOutputFolder" class="bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 px-5 py-2.5 rounded-lg text-sm font-semibold transition-colors dark:text-slate-200">Browse</button>
              </div>
            </div>

            <hr class="border-slate-200 dark:border-slate-700/50 my-4" />

            <!-- Naming Convention -->
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                <label class="text-sm font-semibold text-slate-600 dark:text-slate-400">Append Naming Suffix</label>
                <button 
                  @click="enableSuffix = !enableSuffix"
                  class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-white dark:focus:ring-offset-slate-900"
                  :class="enableSuffix ? 'bg-primary' : 'bg-slate-300 dark:bg-slate-600'"
                >
                  <span 
                    class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform"
                    :class="enableSuffix ? 'translate-x-6' : 'translate-x-1'"
                  />
                </button>
              </div>
              
              <div v-if="enableSuffix" class="animate-in fade-in slide-in-from-top-2">
                <input v-model="suffixInput" type="text" placeholder="e.g. _converted" class="w-full bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-2 focus:ring-primary outline-none">
                <p class="text-xs text-slate-500 mt-2">Example: <code>video.mp4</code> → <code>video{{ suffixInput || '_converted' }}.mp4</code></p>
              </div>
            </div>
          </div>
        </div>

        <!-- Executables -->
        <div class="space-y-4 pt-4 border-t border-slate-200 dark:border-slate-800">
          <h3 class="text-lg font-semibold text-slate-700 dark:text-slate-300">Executable Paths</h3>
          <p class="text-sm text-slate-500 dark:text-slate-400">
            Define explicit paths to your FFmpeg and FFprobe executables if they are not correctly accessible in your system PATH. Leave blank to use defaults.
          </p>

          <div class="space-y-2">
            <label class="text-sm font-semibold text-slate-600 dark:text-slate-400">FFmpeg Executable Path</label>
            <div class="flex gap-2">
              <input v-model="ffmpegPathInput" type="text" placeholder="e.g. C:\ffmpeg\bin\ffmpeg.exe" class="flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 text-sm focus:ring-2 focus:ring-primary outline-none">
              <button @click="browseFfmpeg" class="bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 px-6 py-3 rounded-lg text-sm font-semibold transition-colors dark:text-slate-200">Browse</button>
            </div>
          </div>

          <div class="space-y-2">
            <label class="text-sm font-semibold text-slate-600 dark:text-slate-400">FFprobe Executable Path</label>
            <div class="flex gap-2">
              <input v-model="ffprobePathInput" type="text" placeholder="e.g. C:\ffmpeg\bin\ffprobe.exe" class="flex-1 bg-slate-50 dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 text-sm focus:ring-2 focus:ring-primary outline-none">
              <button @click="browseFfprobe" class="bg-slate-200 dark:bg-slate-800 hover:bg-slate-300 dark:hover:bg-slate-700 px-6 py-3 rounded-lg text-sm font-semibold transition-colors dark:text-slate-200">Browse</button>
            </div>
          </div>
        </div>
        </div>

        <!-- Presets Tab pane -->
        <div v-show="activeTab === 'presets'" class="space-y-4 animate-in fade-in slide-in-from-bottom-2 duration-300">
          <div class="flex items-center justify-between">
            <div>
              <h3 class="text-lg font-semibold text-slate-700 dark:text-slate-300">FFmpeg Presets</h3>
              <p class="text-sm text-slate-500 dark:text-slate-400">
                Manage your custom conversion profiles.
              </p>
            </div>
            <button @click="openPresetModal()" class="flex items-center gap-1.5 bg-primary/10 hover:bg-primary/20 text-primary px-4 py-2 rounded-lg text-sm font-semibold transition-colors">
              <span class="material-symbols-outlined text-[18px]">add</span> Add Preset
            </button>
          </div>

          <div class="bg-white dark:bg-slate-800/50 border border-slate-200 dark:border-slate-700/50 rounded-xl overflow-hidden shadow-sm">
            <div v-if="presets.length === 0" class="p-8 text-center text-slate-500 text-sm">
              No presets found. Add one to get started.
            </div>
            <table v-else class="w-full text-left border-collapse">
              <thead>
                <tr class="bg-slate-50 dark:bg-slate-800 border-b border-slate-200 dark:border-slate-700/50 text-xs font-semibold text-slate-500 uppercase tracking-wider">
                  <th class="px-5 py-3">Preset Name</th>
                  <th class="px-5 py-3">Format</th>
                  <th class="px-5 py-3 text-right">Actions</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-slate-100 dark:divide-slate-800">
                <tr v-for="(preset, index) in presets" :key="index" class="hover:bg-slate-50/50 dark:hover:bg-slate-800/30 transition-colors group">
                  <td class="px-5 py-3">
                    <div class="font-medium text-slate-700 dark:text-slate-200 text-sm">{{ preset.name }}</div>
                    <div class="text-xs text-slate-400 font-mono mt-0.5 truncate max-w-[250px]" :title="preset.args.join(' ')">
                      {{ preset.args.join(' ') }}
                    </div>
                  </td>
                  <td class="px-5 py-3">
                    <span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wider bg-slate-100 dark:bg-slate-700 text-slate-500 dark:text-slate-300">
                      {{ preset.extension }}
                    </span>
                  </td>
                  <td class="px-5 py-3 text-right">
                    <div class="flex items-center justify-end gap-2 opacity-0 group-hover:opacity-100 transition-opacity">
                      <button @click="openPresetModal(index)" class="p-1.5 text-slate-400 hover:text-primary hover:bg-primary/10 rounded transition-colors" title="Edit">
                        <span class="material-symbols-outlined text-[18px]">edit</span>
                      </button>
                      <button @click="deletePreset(index)" class="p-1.5 text-slate-400 hover:text-red-500 hover:bg-red-500/10 rounded transition-colors" title="Delete">
                        <span class="material-symbols-outlined text-[18px]">delete</span>
                      </button>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </div>

      <div class="mt-10 flex items-center justify-end gap-3 pt-6 border-t border-slate-200 dark:border-slate-800">
        <router-link to="/" class="px-6 py-2.5 rounded-lg font-bold text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors">Cancel</router-link>
        <button @click="saveSettings" class="bg-primary hover:bg-primary/90 text-white px-8 py-2.5 rounded-lg font-bold shadow-lg shadow-primary/20 transition-colors">Save Settings</button>
      </div>
    </div>
  </main>

  <!-- Add/Edit Preset Modal -->
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="showPresetModal" class="fixed inset-0 z-50 flex items-center justify-center p-4">
        <div class="absolute inset-0 bg-black/50 backdrop-blur-sm" @click="showPresetModal = false"></div>
        <div class="relative bg-white dark:bg-slate-800 rounded-2xl shadow-xl border border-slate-200 dark:border-slate-700 w-full max-w-lg overflow-hidden flex flex-col max-h-[90vh]">
          
          <div class="px-6 py-4 border-b border-slate-200 dark:border-slate-700 flex items-center justify-between shrink-0">
            <h3 class="font-bold text-lg text-slate-800 dark:text-slate-100">
              {{ editingPresetIndex !== null ? 'Edit Preset' : 'Add New Preset' }}
            </h3>
            <button @click="showPresetModal = false" class="text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-700 p-1.5 rounded-lg transition-colors">
              <span class="material-symbols-outlined text-[20px]">close</span>
            </button>
          </div>
          
          <div class="p-6 overflow-y-auto space-y-5">
            <div class="space-y-4">
              <div class="grid grid-cols-3 gap-4">
                <div class="col-span-2 space-y-1.5">
                  <label class="text-sm font-semibold text-slate-700 dark:text-slate-300">Preset Name</label>
                  <input v-model="presetForm.name" type="text" placeholder="e.g. H.264 Fast 1080p" class="w-full bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-2 focus:ring-primary outline-none">
                </div>
                <div class="space-y-1.5">
                  <label class="text-sm font-semibold text-slate-700 dark:text-slate-300">Format</label>
                  <input v-model="presetForm.extension" type="text" placeholder="mp4" class="w-full bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-2.5 text-sm focus:ring-2 focus:ring-primary outline-none uppercase placeholder:normal-case font-mono">
                </div>
              </div>
              
              <div class="space-y-1.5">
                <label class="text-sm font-semibold text-slate-700 dark:text-slate-300 flex justify-between">
                  <span>FFmpeg Arguments</span>
                </label>
                <textarea v-model="presetForm.args" rows="4" placeholder="-c:v libx264 -preset fast -crf 22 -c:a aac -b:a 128k" class="w-full bg-slate-50 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded-lg px-4 py-3 text-sm focus:ring-2 focus:ring-primary outline-none font-mono resize-none"></textarea>
                <p class="text-xs text-slate-500 mt-1 flex items-start gap-1">
                  <span class="material-symbols-outlined text-[14px]">info</span>
                  Provide space-separated raw arguments. Input `-i` scaling and Output path mapping are handled automatically by the engine.
                </p>
              </div>
            </div>
          </div>
          
          <div class="px-6 py-4 border-t border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-800/50 flex justify-end gap-3 shrink-0">
            <button @click="showPresetModal = false" class="px-5 py-2.5 text-slate-600 dark:text-slate-300 font-semibold hover:bg-slate-200 dark:hover:bg-slate-700 rounded-lg transition-colors">
              Cancel
            </button>
            <button @click="savePreset" class="px-6 py-2.5 bg-primary text-white font-bold rounded-lg shadow-lg hover:bg-primary/90 transition-colors">
              Save Preset
            </button>
          </div>
          
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
