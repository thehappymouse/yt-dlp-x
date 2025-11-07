export const extractErrorMessage = (error) => {
  if (!error) return "未知错误";
  if (typeof error === "string") return error;
  if (typeof error === "object") {
    if ("message" in error && error.message) {
      return error.message;
    }
    try {
      return JSON.stringify(error);
    } catch (serializationError) {
      return String(error);
    }
  }

  return String(error);
};
