export const rejectError = (res: Response) =>
  res.status >= 200 && res.status < 300
    ? Promise.resolve(res)
    : Promise.reject(res);
