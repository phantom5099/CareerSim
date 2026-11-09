export class ApiError extends Error {
  readonly status: number
  readonly code: string

  constructor(message: string, status = 0, code = 'API_ERROR') {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}
