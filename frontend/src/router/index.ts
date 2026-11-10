import { createRouter, createWebHistory } from 'vue-router'

import AppLayout from '@/layouts/AppLayout.vue'
import ApplicationsPage from '@/pages/ApplicationsPage.vue'
import CompaniesPage from '@/pages/CompaniesPage.vue'
import CompanyDetailPage from '@/pages/CompanyDetailPage.vue'
import ImportCompanyPage from '@/pages/ImportCompanyPage.vue'
import InterviewRoomPage from '@/pages/InterviewRoomPage.vue'
import LoginPage from '@/pages/LoginPage.vue'
import OffersPage from '@/pages/OffersPage.vue'
import ResumePage from '@/pages/ResumePage.vue'
import { useApplicationsStore } from '@/stores/applications'
import { useAuthStore } from '@/stores/auth'
import { useCompanyCatalogStore } from '@/stores/companyCatalog'
import { useOffersStore } from '@/stores/offers'
import { useResumesStore } from '@/stores/resumes'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/login',
      name: 'login',
      component: LoginPage,
      meta: { public: true, title: '登录' },
    },
    {
      path: '/',
      component: AppLayout,
      children: [
        {
          path: '',
          name: 'companies',
          component: CompaniesPage,
          meta: { title: '在招公司' },
        },
        {
          path: 'companies/import',
          name: 'companyImport',
          component: ImportCompanyPage,
          meta: { title: '录入公司模板' },
        },
        {
          path: 'companies/:id',
          name: 'companyDetail',
          component: CompanyDetailPage,
          meta: { title: '公司档案' },
        },
        {
          path: 'applications',
          name: 'applications',
          component: ApplicationsPage,
          meta: { title: '面试进度' },
        },
        {
          path: 'resume',
          name: 'resume',
          component: ResumePage,
          meta: { title: '简历管理' },
        },
        {
          path: 'interview/:applicationId',
          name: 'interview',
          component: InterviewRoomPage,
          meta: { title: '面试记录' },
        },
        {
          path: 'offers',
          name: 'offers',
          component: OffersPage,
          meta: { title: 'Offer 清单' },
        },
      ],
    },
    {
      path: '/:pathMatch(.*)*',
      redirect: '/',
    },
  ],
})

router.beforeEach(async (to) => {
  const auth = useAuthStore()

  if (to.meta.public) {
    if (auth.token && (await auth.bootstrap())) {
      return { name: 'companies' }
    }
    return true
  }

  if (!(await auth.bootstrap())) {
    return {
      name: 'login',
      query: to.fullPath === '/' ? {} : { redirect: to.fullPath },
    }
  }

  await Promise.all([
    useCompanyCatalogStore().init(),
    useApplicationsStore().init(),
    useOffersStore().init(),
    useResumesStore().init(),
  ])

  return true
})

router.afterEach((to) => {
  const title = typeof to.meta.title === 'string' ? to.meta.title : ''
  document.title = title ? `${title} · CareerSim` : 'CareerSim · 求职模拟器'
})

export default router
