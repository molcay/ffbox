import { reactive } from 'vue'

export const globalState = reactive({
    isConverting: false,
    conversionProgress: 0,
    conversionStatus: 'Waiting for input...',
    currentFileProgress: '',
    isDependenciesReady: false,
    isCheckingDependencies: true,
    isDownloadingDependencies: false,
    downloadStatus: { status: '', percentage: 0 }
})
