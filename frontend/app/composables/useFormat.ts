import type { BadgeProps } from '@nuxt/ui'

type RegionType = 'bulk' | 'pickface' | 'putaway' | 'processing' | 'order'

const REGION_COLORS: Record<RegionType, BadgeProps['color']> = {
  bulk: 'primary',
  pickface: 'success',
  putaway: 'warning',
  processing: 'info',
  order: 'secondary',
}

export function useFormat() {
  const formatItemKind = (itemKind: string) =>
    itemKind.replace(/^[^:]+:/, '').replaceAll('_', ' ')

  const formatCount = (value: number) => {
    const absolute = Math.abs(value)
    if (absolute >= 1_000_000) return `${(value / 1_000_000).toFixed(absolute >= 10_000_000 ? 0 : 1).replace(/\.0$/, '')}m`
    if (absolute >= 1_000) return `${(value / 1_000).toFixed(absolute >= 10_000 ? 0 : 1).replace(/\.0$/, '')}k`
    return new Intl.NumberFormat('en-US').format(value)
  }

  const regionColor = (type: string): BadgeProps['color'] =>
    REGION_COLORS[type as RegionType] ?? 'neutral'

  return { formatItemKind, formatCount, regionColor }
}
