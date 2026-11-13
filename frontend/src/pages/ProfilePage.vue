<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'

import { useAuthStore } from '@/stores/auth'

const auth = useAuthStore()

// ---- 资料表单 ----

const profileForm = reactive({
  displayName: '',
  school: '',
  major: '',
  graduationYear: '',
  targetRoles: '',
  targetCities: '',
})

const profileTouched = ref(false)
const profileSaved = ref(false)

function fillFromUser(): void {
  const user = auth.user
  if (!user) {
    return
  }
  profileForm.displayName = user.displayName
  profileForm.school = user.school ?? ''
  profileForm.major = user.major ?? ''
  profileForm.graduationYear = user.graduationYear === null ? '' : String(user.graduationYear)
  profileForm.targetRoles = user.targetRoles.join('、')
  profileForm.targetCities = user.targetCities.join('、')
}

watch(() => auth.user, fillFromUser, { immediate: true })

const profileError = computed(() => {
  if (!profileTouched.value) {
    return ''
  }
  const displayNameLength = [...profileForm.displayName.trim()].length
  if (displayNameLength === 0 || displayNameLength > 40) {
    return '昵称需 1-40 个字符'
  }
  if ([...profileForm.school].length > 80 || [...profileForm.major].length > 80) {
    return '学校/专业不能超过 80 个字符'
  }
  if (profileForm.graduationYear !== '') {
    const year = Number(profileForm.graduationYear)
    if (!Number.isInteger(year) || year < 1980 || year > 2100) {
      return '毕业年份需在 1980-2100 之间'
    }
  }
  return ''
})

const canSaveProfile = computed(() => profileError.value === '' && !auth.loading)

/** 中日韩或顿号分隔的输入拆成列表；空串返回空数组以清空列表。 */
function splitList(raw: string): string[] {
  return raw
    .split(/[、,，]/)
    .map((item) => item.trim())
    .filter((item) => item.length > 0)
}

async function saveProfile() {
  profileTouched.value = true
  profileSaved.value = false
  if (!canSaveProfile.value) {
    return
  }

  const year = profileForm.graduationYear === '' ? null : Number(profileForm.graduationYear)
  const ok = await auth.updateProfile({
    displayName: profileForm.displayName.trim(),
    school: profileForm.school.trim() === '' ? null : profileForm.school.trim(),
    major: profileForm.major.trim() === '' ? null : profileForm.major.trim(),
    graduationYear: year,
    targetRoles: splitList(profileForm.targetRoles),
    targetCities: splitList(profileForm.targetCities),
  })
  profileSaved.value = ok
  if (ok) {
    // 表单同步为服务端返回的规范化数据。
    fillFromUser()
  }
}

// ---- 改密表单 ----

const passwordForm = reactive({
  currentPassword: '',
  newPassword: '',
  confirmPassword: '',
})
const passwordTouched = ref(false)
const passwordSaved = ref(false)
const showNewPassword = ref(false)

const passwordError = computed(() => {
  if (!passwordTouched.value) {
    return ''
  }
  if (passwordForm.currentPassword.length === 0) {
    return '请输入当前密码'
  }
  if (passwordForm.newPassword.length < 8) {
    return '新密码至少 8 位'
  }
  if (passwordForm.newPassword.length > 128) {
    return '新密码最长 128 位'
  }
  if (passwordForm.newPassword !== passwordForm.confirmPassword) {
    return '两次输入的新密码不一致'
  }
  return ''
})

const canSavePassword = computed(() => passwordError.value === '' && !auth.loading)

async function savePassword() {
  passwordTouched.value = true
  passwordSaved.value = false
  if (!canSavePassword.value) {
    return
  }

  const ok = await auth.changePassword(passwordForm.currentPassword, passwordForm.newPassword)
  passwordSaved.value = ok
  if (ok) {
    passwordForm.currentPassword = ''
    passwordForm.newPassword = ''
    passwordForm.confirmPassword = ''
    passwordTouched.value = false
  }
}
</script>

<template>
  <div class="mx-auto max-w-2xl">
    <header class="border-b border-line pb-5">
      <p class="font-mono text-[0.56rem] tracking-[0.3em] text-ink-faint">PROFILE / 我的档案</p>
      <h1 class="mt-2 font-display text-3xl leading-tight">个人资料</h1>
      <p class="mt-2 text-xs leading-5 text-ink-soft">
        资料用于面试官开场白与上下文定制；留空表示不提供。
      </p>
    </header>

    <section class="mt-6 border border-ink bg-paper-light">
      <div
        class="flex items-center justify-between border-b border-line px-5 py-3 text-[0.64rem] text-ink-faint"
      >
        <span>资料 / PROFILE</span>
        <span>{{ auth.user?.username }}</span>
      </div>

      <form class="space-y-5 px-5 py-6" novalidate @submit.prevent="saveProfile">
        <div class="grid gap-5 sm:grid-cols-2">
          <div>
            <label for="displayName" class="mb-1.5 block text-[0.8rem] text-ink">昵称</label>
            <input
              id="displayName"
              v-model="profileForm.displayName"
              class="field"
              type="text"
              autocomplete="nickname"
            />
          </div>
          <div>
            <label for="graduationYear" class="mb-1.5 block text-[0.8rem] text-ink">
              毕业年份
            </label>
            <input
              id="graduationYear"
              v-model="profileForm.graduationYear"
              class="field"
              type="number"
              min="1980"
              max="2100"
              placeholder="如 2027"
            />
          </div>
          <div>
            <label for="school" class="mb-1.5 block text-[0.8rem] text-ink">学校</label>
            <input id="school" v-model="profileForm.school" class="field" type="text" />
          </div>
          <div>
            <label for="major" class="mb-1.5 block text-[0.8rem] text-ink">专业</label>
            <input id="major" v-model="profileForm.major" class="field" type="text" />
          </div>
          <div>
            <label for="targetRoles" class="mb-1.5 block text-[0.8rem] text-ink">
              目标岗位（顿号分隔）
            </label>
            <input
              id="targetRoles"
              v-model="profileForm.targetRoles"
              class="field"
              type="text"
              placeholder="后端工程师、数据分析"
            />
          </div>
          <div>
            <label for="targetCities" class="mb-1.5 block text-[0.8rem] text-ink">
              目标城市（顿号分隔）
            </label>
            <input
              id="targetCities"
              v-model="profileForm.targetCities"
              class="field"
              type="text"
              placeholder="上海、杭州"
            />
          </div>
        </div>

        <p v-if="profileError" role="alert" class="text-xs text-vermilion">{{ profileError }}</p>
        <p v-if="auth.error" role="alert" class="text-xs text-vermilion">{{ auth.error }}</p>
        <p v-if="profileSaved" class="text-xs text-ink-soft">资料已更新。</p>

        <button type="submit" class="btn-solid" :disabled="!canSaveProfile">
          {{ auth.loading ? '保存中…' : '保存资料' }}
        </button>
      </form>
    </section>

    <section class="mt-6 border border-ink bg-paper-light">
      <div class="border-b border-line px-5 py-3 text-[0.64rem] text-ink-faint">
        修改密码 / PASSWORD
      </div>

      <form class="space-y-5 px-5 py-6" novalidate @submit.prevent="savePassword">
        <div>
          <label for="currentPassword" class="mb-1.5 block text-[0.8rem] text-ink">当前密码</label>
          <input
            id="currentPassword"
            v-model="passwordForm.currentPassword"
            class="field"
            type="password"
            autocomplete="current-password"
          />
        </div>
        <div>
          <label for="newPassword" class="mb-1.5 block text-[0.8rem] text-ink">新密码</label>
          <div class="relative">
            <input
              id="newPassword"
              v-model="passwordForm.newPassword"
              class="field pr-11"
              :type="showNewPassword ? 'text' : 'password'"
              autocomplete="new-password"
              placeholder="至少 8 位"
            />
            <button
              type="button"
              class="absolute inset-y-0 right-0 flex items-center px-3 text-ink-faint transition hover:text-ink"
              :aria-label="showNewPassword ? '隐藏密码' : '显示密码'"
              @click="showNewPassword = !showNewPassword"
            >
              <svg
                viewBox="0 0 16 16"
                class="h-4 w-4"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <path d="M1.5 8S4 3.9 8 3.9 14.5 8 14.5 8 12 12.1 8 12.1 1.5 8 1.5 8Z" />
                <circle cx="8" cy="8" r="2.1" />
                <path v-if="showNewPassword" d="M2.6 2.6l10.8 10.8" />
              </svg>
            </button>
          </div>
        </div>
        <div>
          <label for="confirmNewPassword" class="mb-1.5 block text-[0.8rem] text-ink">
            确认新密码
          </label>
          <input
            id="confirmNewPassword"
            v-model="passwordForm.confirmPassword"
            class="field"
            type="password"
            autocomplete="new-password"
          />
        </div>

        <p v-if="passwordError" role="alert" class="text-xs text-vermilion">
          {{ passwordError }}
        </p>
        <p v-if="auth.error" role="alert" class="text-xs text-vermilion">{{ auth.error }}</p>
        <p v-if="passwordSaved" class="text-xs text-ink-soft">
          密码已修改；其他设备的登录已全部退出。
        </p>

        <button type="submit" class="btn-solid" :disabled="!canSavePassword">
          {{ auth.loading ? '提交中…' : '修改密码' }}
        </button>
      </form>
    </section>
  </div>
</template>
