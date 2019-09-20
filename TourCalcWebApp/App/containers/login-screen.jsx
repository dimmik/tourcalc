import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';

export default class LoginScreen extends React.Component {
    constructor(props) {
        super(props);
    }
    scope = "none"
    code = "none"

    render() {
        return (
            <div>Login Screen:
              <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this.props.app, this.scope, this.code)
                }}>
                    <p>scope:</p>
                    <input
                        type='text'
                        onChange={(e) => this.scope = event.target.value}
                    />
                    <p>code:</p>
                    <input
                        type='text'
                        onChange={(e) => this.code = event.target.value}
                    />
                    <input
                        type='submit'
                    />
                </form>
            
                </div>
            )
    }
    
};
