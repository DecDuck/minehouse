import createClient from 'openapi-fetch'
import type { paths } from '../api-types'

export function useMinehouseApi() {
  const config = useRuntimeConfig()
  return createClient<paths>({ baseUrl: config.public.apiBase })
}
