import { client } from "./client"

export class LoginService {
  public static loginAccessToken(options) {
    return client.post({ url: "/api/v1/login/access-token", ...options })
  }
}
