import { createRouter, createWebHistory } from 'vue-router'
import TimelineView from '../views/TimelineView.vue'
import SettingsView from '../views/SettingsView.vue'
import AboutView from '../views/AboutView.vue'

const WeekView = () => import('../views/WeekView.vue')

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', component: TimelineView },
    { path: '/week', component: WeekView },
    { path: '/settings', component: SettingsView },
    { path: '/about', component: AboutView },
  ],
})
