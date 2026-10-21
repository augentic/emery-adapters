import { zodResolver } from "@hookform/resolvers/zod"
import { createFileRoute, Link, redirect } from "@tanstack/react-router"
import { useForm } from "react-hook-form"
import { z } from "zod"

import useAuth, { isLoggedIn } from "@/hooks/useAuth"

const schema = z.object({
  username: z.email({ message: "Invalid email" }),
  password: z
    .string()
    .min(8, { message: "Too short" }),
})

export const Route = createFileRoute("/login")({
  component: Login,
  beforeLoad: async () => {
    if (isLoggedIn()) {
      throw redirect({ to: "/" })
    }
  },
})

function Login() {
  const { loginMutation } = useAuth()
  const form = useForm({
    resolver: zodResolver(schema),
    defaultValues: { username: "", password: "" },
  })
  const onSubmit = (data) => {
    if (loginMutation.isPending) return
    loginMutation.mutate(data)
  }
  return (
    <form onSubmit={form.handleSubmit(onSubmit)}>
      <input name="username" type="email" />
      <button type="submit">Log In</button>
      <Link to="/signup">Sign up</Link>
    </form>
  )
}
