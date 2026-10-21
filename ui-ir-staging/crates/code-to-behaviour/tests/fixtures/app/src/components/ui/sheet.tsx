import * as React from "react"
import * as SheetPrimitive from "@radix-ui/react-dialog"

const Sheet = SheetPrimitive.Root
const SheetTrigger = SheetPrimitive.Trigger

const SheetContent = React.forwardRef(({ children, ...props }, ref) => (
  <SheetPrimitive.Portal>
    <SheetPrimitive.Content ref={ref} {...props}>
      {children}
      <SheetPrimitive.Close>
        <span className="sr-only">Close</span>
      </SheetPrimitive.Close>
    </SheetPrimitive.Content>
  </SheetPrimitive.Portal>
))

export { Sheet, SheetContent, SheetTrigger }
