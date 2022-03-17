TARGET = hoshinova

$(TARGET):
	go build -tags netgo -o $(TARGET)
	upx $(TARGET)
