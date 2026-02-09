/**
 * i18n Configuration for Oxide Lab
 *
 * Настройка sveltekit-i18n с поддержкой ICU MessageFormat для плюрализации
 * и форматирования сообщений. Использует асинхронные loaders для загрузки
 * переводов по требованию.
 */

import i18n from '@sveltekit-i18n/base';
import parserIcu from '@sveltekit-i18n/parser-icu';
import type { Config } from '@sveltekit-i18n/parser-icu';

const config: Config = {
    // Парсер ICU для поддержки плюрализации и форматирования
    parser: parserIcu(),

    // Локаль по умолчанию
    initLocale: 'en',

    // Fallback локаль при отсутствии перевода
    fallbackLocale: 'en',

    // Загрузчики переводов для каждой локали
    loaders: [
        // English (en) - общие переводы
        {
            locale: 'en',
            key: 'common',
            loader: async () => (await import('./locales/en/common.json')).default,
        },
        {
            locale: 'en',
            key: 'sidebar',
            loader: async () => (await import('./locales/en/sidebar.json')).default,
        },
        {
            locale: 'en',
            key: 'settings',
            loader: async () => (await import('./locales/en/settings.json')).default,
        },
        {
            locale: 'en',
            key: 'models',
            loader: async () => (await import('./locales/en/models.json')).default,
        },
        {
            locale: 'en',
            key: 'chat',
            loader: async () => (await import('./locales/en/chat.json')).default,
        },
        {
            locale: 'en',
            key: 'errors',
            loader: async () => (await import('./locales/en/errors.json')).default,
        },
        {
            locale: 'en',
            key: 'about',
            loader: async () => (await import('./locales/en/about.json')).default,
        },

        // Russian (ru) - общие переводы
        {
            locale: 'ru',
            key: 'common',
            loader: async () => (await import('./locales/ru/common.json')).default,
        },
        {
            locale: 'ru',
            key: 'sidebar',
            loader: async () => (await import('./locales/ru/sidebar.json')).default,
        },
        {
            locale: 'ru',
            key: 'settings',
            loader: async () => (await import('./locales/ru/settings.json')).default,
        },
        {
            locale: 'ru',
            key: 'models',
            loader: async () => (await import('./locales/ru/models.json')).default,
        },
        {
            locale: 'ru',
            key: 'chat',
            loader: async () => (await import('./locales/ru/chat.json')).default,
        },
        {
            locale: 'ru',
            key: 'errors',
            loader: async () => (await import('./locales/ru/errors.json')).default,
        },
        {
            locale: 'ru',
            key: 'about',
            loader: async () => (await import('./locales/ru/about.json')).default,
        },

        // Portuguese (pt-BR) - общие переводы
        {
            locale: 'pt-BR',
            key: 'common',
            loader: async () => (await import('./locales/pt-BR/common.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'sidebar',
            loader: async () => (await import('./locales/pt-BR/sidebar.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'settings',
            loader: async () => (await import('./locales/pt-BR/settings.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'models',
            loader: async () => (await import('./locales/pt-BR/models.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'chat',
            loader: async () => (await import('./locales/pt-BR/chat.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'errors',
            loader: async () => (await import('./locales/pt-BR/errors.json')).default,
        },
        {
            locale: 'pt-BR',
            key: 'about',
            loader: async () => (await import('./locales/pt-BR/about.json')).default,
        },
    ],

    // Настройки логирования (только ошибки в продакшене)
    log: {
        level: 'error',
        prefix: '[i18n]: ',
    },

    // Обработка отсутствующих ключей
    fallbackValue: (key: string) => {
        if (import.meta.env.DEV) {
            console.warn(`[i18n] Missing translation key: ${key}`);
        }
        return key;
    },
};

export const { t, locale, locales, loading, loadTranslations } = new i18n(config);
