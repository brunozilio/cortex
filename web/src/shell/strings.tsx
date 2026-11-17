import { createContext, useContext } from 'react'
import { EN, type Strings } from '@/lib/i18n'

const StringsContext = createContext<Strings>(EN)

export const StringsProvider = StringsContext.Provider

export function useStrings(): Strings {
  return useContext(StringsContext)
}
