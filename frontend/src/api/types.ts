export interface User {
  id: string
  username: string
  displayName: string
  school?: string
  major?: string
  graduationYear?: number
  targetRoles: string[]
  targetCities: string[]
  avatarText: string
  avatarColor: string
}

export interface AuthSession {
  token: string
  expiresAt: string
  user: User
}

export interface LoginPayload {
  username: string
  password: string
}

export interface ApiErrorBody {
  error: {
    code: string
    message: string
    requestId: string
  }
}

export interface IndustriesResponse {
  industries: string[]
}
