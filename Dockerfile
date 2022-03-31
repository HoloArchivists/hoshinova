FROM golang:latest as builder
# Installing UPX and GIT for building hoshinova and ytarchive 
RUN apt update
RUN apt install -y upx git
# Building Hoshinova
WORKDIR /app/hoshinova
COPY . .
RUN make
# Cloning and Building ytarchive
RUN git clone https://github.com/Kethsar/ytarchive.git /app/ytarchive
WORKDIR /app/ytarchive
RUN go build

FROM alpine:latest
RUN mkdir /app
# Copying executables
COPY --from=builder /app/hoshinova/hoshinova /app/
COPY --from=builder /app/ytarchive/ytarchive /app/
# Adding ytarchive and hoshinova to PATH
ENV PATH /app/:$PATH
# Going to config folder
WORKDIR /config/
CMD ls && hoshinova

