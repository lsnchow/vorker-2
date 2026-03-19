"use client"

import { createContext, useContext, useState, ReactNode } from "react"

export type ThemeColor = "green" | "red" | "blue" | "yellow"

interface ThemeContextType {
  themeColor: ThemeColor
  setThemeColor: (color: ThemeColor) => void
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined)

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeColor, setThemeColor] = useState<ThemeColor>("green")

  return (
    <ThemeContext.Provider value={{ themeColor, setThemeColor }}>
      <div data-theme={themeColor}>{children}</div>
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider")
  }
  return context
}
