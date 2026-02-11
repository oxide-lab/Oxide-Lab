/**
 * Services Module - Central export for all services
 *
 * This module re-exports all service classes and functions used throughout the application.
 */

// Backend integration
export { initializeBackend, cleanupBackend, isBackendInitialized } from './backend';

// Local models service
export { LocalModelsService } from './local-models';

// Model cards service
export { ModelCardsService } from './model-cards';

// Performance service
export { PerformanceService, performanceService } from './performance-service';

// Hardware service
export { hardwareService } from './hardware-service';

// Llama backend service
export { llamaBackendService } from './llama-backend-service';

// App updater service
export {
  checkForAppUpdate,
  downloadAndInstallUpdate,
  getUpdateCheckIntervalMs,
  isAutoUpdaterDisabledByEnv,
  type AppUpdateHandle,
  type AppUpdateInfo,
  type AppUpdateProgressEvent,
} from './app-updater';


