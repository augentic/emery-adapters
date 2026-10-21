import { createFileRoute } from "@tanstack/react-router"

import { SignIn } from "@/features/sign-in"

export const Route = createFileRoute("/(auth)/sign-in")({
  component: SignIn,
})
