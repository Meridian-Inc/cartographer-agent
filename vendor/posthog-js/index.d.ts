export interface PostHogInitOptions {
  api_host?: string
  defaults?: string
  capture_pageview?: boolean
  capture_pageleave?: boolean
}

export interface PostHog {
  init(apiKey: string, options?: PostHogInitOptions): void
  capture(event: string, properties?: Record<string, unknown>): void
  identify(distinctId: string, properties?: Record<string, unknown>): void
  reset(): void
  opt_out_capturing(): void
  opt_in_capturing(): void
}

declare const posthog: PostHog
export default posthog
