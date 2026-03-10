import { createRouter, createWebHistory } from 'vue-router'
import HomePage from './pages/HomePage.vue'
import SettingsPage from './pages/SettingsPage.vue'

const routes = [
    {
        path: '/',
        name: 'Home',
        component: HomePage
    },
    {
        path: '/settings',
        name: 'Settings',
        component: SettingsPage
    }
]

export const router = createRouter({
    history: createWebHistory(),
    routes
})
