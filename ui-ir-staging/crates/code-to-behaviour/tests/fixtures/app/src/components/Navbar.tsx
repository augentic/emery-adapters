import { useState } from "react"

import { Sheet, SheetContent, SheetTrigger } from "@/components/ui/sheet"

const routes = [
  { href: "#features", label: "Features" },
  { href: "#faq", label: "FAQ" },
]

export const Navbar = () => {
  const [isOpen, setIsOpen] = useState(false)
  return (
    <header>
      <a href="/">Home</a>
      <Sheet open={isOpen} onOpenChange={setIsOpen}>
        <SheetTrigger>Menu</SheetTrigger>
        <SheetContent>
          {routes.map(({ href, label }) => (
            <a key={label} href={href} onClick={() => setIsOpen(false)}>
              {label}
            </a>
          ))}
        </SheetContent>
      </Sheet>
      <a href="https://example.com" target="_blank">
        Docs
      </a>
    </header>
  )
}
