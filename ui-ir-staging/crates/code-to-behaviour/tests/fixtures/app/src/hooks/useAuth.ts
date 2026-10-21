import { useMutation } from "@tanstack/react-query"
import { useNavigate } from "@tanstack/react-router"
import { toast } from "sonner"

import { LoginService } from "@/client"

const isLoggedIn = () => localStorage.getItem("access_token") !== null

const useAuth = () => {
  const navigate = useNavigate()
  const login = async (data) => {
    const response = await LoginService.loginAccessToken({ formData: data })
    localStorage.setItem("access_token", response.access_token)
  }
  const fail = function (this: (m: string) => void) {
    this("Login failed")
  }
  const showError = (description) => toast.error(description)
  const loginMutation = useMutation({
    mutationFn: login,
    onSuccess: () => {
      navigate({ to: "/" })
    },
    onError: fail.bind(showError),
  })
  return { loginMutation }
}

export { isLoggedIn }
export default useAuth
