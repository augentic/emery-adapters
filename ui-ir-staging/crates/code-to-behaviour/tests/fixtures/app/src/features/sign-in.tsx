import { zodResolver } from "@hookform/resolvers/zod"
import { createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { useForm } from "react-hook-form"
import { z } from "zod"

const schema = z.object({
  email: z.string().min(1, "Please enter your email.").email("Not an email"),
  code: z.string().min(6, { error: "Six digits" }),
})

const Route = createFileRoute("/(auth)/sign-in")({})

export function SignIn() {
  const navigate = Route.useNavigate()
  const form = useForm({ resolver: zodResolver(schema) })
  const [show, setShow] = useState(false)
  const onSubmit = () => {
    navigate({ to: "/home" })
  }
  return (
    <form onSubmit={form.handleSubmit(onSubmit)}>
      <input name="nickname" required minLength={3} />
      <button type="button" onClick={() => setShow(true)}>{show ? "Hide" : "Show"}</button>
      <button>Sign in</button>
    </form>
  )
}
