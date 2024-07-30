from http.server import BaseHTTPRequestHandler, HTTPServer

class RequestHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        # 处理 GET 请求
        self.send_response(200)
        self.send_header('Content-type', 'text/plain')
        self.end_headers()

        # 打印请求信息
        print(f"Received request from {self.client_address[0]}:{self.client_address[1]}")
        print(f"Path: {self.path}")
        print("Headers:")
        for header, value in self.headers.items():
            print(f"  {header}: {value}")

        # 发送响应
        self.wfile.write("Hello, this is the server!".encode('utf-8'))

def run(server_class=HTTPServer, handler_class=RequestHandler, port=9200):
    server_address = ('', port)
    httpd = server_class(server_address, handler_class)
    print(f"Server listening on port {port}")
    httpd.serve_forever()

if __name__ == '__main__':
    run()
